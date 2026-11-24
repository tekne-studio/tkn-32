//! A cartridge's file, prepared once before any engine runs it: held to the
//! contract's limits, cut to the contract's exports, and given a meter of
//! work and a count of its stack written into its code. Every engine runs
//! the prepared bytes, so a cartridge stops at the same instruction on each.
//!
//! The meter is one exported `i32` global. At a function's start, its
//! declared locals and the instructions it holds outside the loops inside
//! it are taken from it; at the top of each turn of a loop, the
//! instructions the loop holds outside the loops inside it; a bulk memory
//! instruction takes one unit per [`BULK_BYTES`] it moves. Below zero the
//! code traps. Control flow is structured, so an instruction runs at most
//! once a turn of the innermost loop around it, or once a call when no loop
//! is: the charge covers every path, the longest whole, and no loop and no
//! recursion runs uncounted.
//!
//! The stack is a second global, the cartridge's own and not exported. A
//! call adds its callee's frame and [`FRAME_OVERHEAD`] to it before, and
//! takes them back after; past [`MAX_STACK`] the code sets the meter to
//! [`TOO_DEEP`] and traps. An indirect call counts the largest frame of a
//! function of its type.
//!
//! A start function is exported as [`START`] for the runner to call before
//! `cart_abi`, under the meter's first value, [`START_BUDGET`]: its stop
//! reads as any other's, never as an engine's failure to instantiate.

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

use cartridge::{
    BULK_BYTES, Export, FRAME_OVERHEAD, MAX_FILE, MAX_FRAME, MAX_MEMORY, MAX_STACK, MAX_TABLE,
    MEMORY, START_BUDGET, Signature, TONE, TONE_SIGNATURE,
};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, Encode, ExportKind, ExportSection, GlobalSection,
    GlobalType, Instruction, MemorySection, MemoryType, Module, RawSection, SectionId, ValType,
};
use wasmparser::types::{Types, TypesRef};
use wasmparser::{
    DataKind, DataSectionReader, ElementItems, ElementKind, ElementSectionReader, ExternalKind,
    FuncType, FuncValidator, FuncValidatorAllocations, FunctionBody, Operator, Parser, Payload,
    TypeRef, ValidPayload, Validator, ValidatorResources, WasmFeatures,
};

/// The name the prepared file exports its meter by.
pub const METER: &str = "meter";
/// The name the prepared file exports its start function by, if it has one.
pub const START: &str = "start";
/// What the meter holds after a cartridge asked for memory past its
/// ceiling: lower than any charge takes it.
pub const OUT_OF_MEMORY: i32 = i32::MIN;
/// What the meter holds after a cartridge called deeper than
/// [`MAX_STACK`]: lower than any charge takes it.
pub const TOO_DEEP: i32 = i32::MIN + 1;

/// What a cartridge may use: WebAssembly 2.0 without SIMD, which wasmi and
/// every browser run alike. Threads, tail calls, exceptions, a second
/// memory, a 64-bit memory, extended constants, custom page sizes and GC
/// are refused.
const FEATURES: WasmFeatures = WasmFeatures::WASM2.difference(WasmFeatures::SIMD);
const PAGE: u64 = 64 * 1024;

/// A cartridge's file as every engine runs it.
#[derive(Clone, Debug)]
pub struct Prepared {
    bytes: Vec<u8>,
}

impl Prepared {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Why a file is not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Its size, over [`MAX_FILE`].
    TooLarge(usize),
    /// The validator's words: not WebAssembly, or a feature outside the set.
    Invalid(String),
    /// An import other than `env.tone` of the contract's type, as
    /// `module.name`.
    Import(String),
    /// A name of the contract missing, or not of the contract's type.
    Export(&'static str),
    /// Not one memory of its own, of 32-bit addresses, unshared, starting
    /// within [`MAX_MEMORY`].
    Memory,
    /// More than one table, or one of more than [`MAX_TABLE`] elements.
    Table,
    /// A function whose frame holds more than [`MAX_FRAME`] values: how
    /// many it holds.
    Frame(u32),
    /// A data or element segment that does not fit its memory or its table.
    Segment,
    /// An instruction the meter has no price for.
    Instruction(&'static str),
}

impl Refusal {
    /// A word for why, as a host answers a file it will not keep: the
    /// sentence is [`Display`](fmt::Display)'s.
    pub const fn code(&self) -> &'static str {
        match self {
            Refusal::TooLarge(_) => "size",
            Refusal::Invalid(_) => "invalid",
            Refusal::Import(_) => "import",
            Refusal::Export(_) => "export",
            Refusal::Memory => "memory",
            Refusal::Table => "table",
            Refusal::Frame(_) => "frame",
            Refusal::Segment => "segment",
            Refusal::Instruction(_) => "instruction",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::TooLarge(size) => write!(
                f,
                "it is {} KB, over {} KB",
                size.div_ceil(1024),
                MAX_FILE / 1024
            ),
            Refusal::Invalid(why) => write!(f, "not a cartridge: {why}"),
            Refusal::Import(name) => write!(f, "it imports {name}"),
            Refusal::Export(name) => write!(f, "no {name} of the contract's type"),
            Refusal::Memory => write!(
                f,
                "its memory is not its own or over {} KB",
                MAX_MEMORY / 1024
            ),
            Refusal::Table => write!(f, "more than one table, or one over {MAX_TABLE}"),
            Refusal::Frame(values) => {
                write!(f, "a frame of {values} values, over {MAX_FRAME}")
            }
            Refusal::Segment => write!(f, "a segment outside its memory or its table"),
            Refusal::Instruction(name) => write!(f, "it uses {name}"),
        }
    }
}

/// Holds `file` to the contract's limits and writes the meter and the
/// stack's count into it.
pub fn prepare(file: &[u8]) -> Result<Prepared, Refusal> {
    if file.len() > MAX_FILE {
        return Err(Refusal::TooLarge(file.len()));
    }
    let survey = survey(file)?;
    let types = survey.types.as_ref();
    let memory = (types.memory_count() == 1)
        .then(|| types.memory_at(0))
        .ok_or(Refusal::Memory)?;
    let pages = MAX_MEMORY as u64 / PAGE;
    if memory.memory64 || memory.shared || memory.initial > pages {
        return Err(Refusal::Memory);
    }
    let table = match types.table_count() {
        0 => 0,
        1 => types.table_at(0).initial,
        _ => return Err(Refusal::Table),
    };
    if table > u64::from(MAX_TABLE) {
        return Err(Refusal::Table);
    }

    // Imports are refused but for `env.tone`, so the meter and the stack
    // are the globals after the module's own, and a function's index counts
    // those imports.
    let counts = Counts::new(types, &survey.frames);
    let mut imported = 0;
    let mut out = Module::new();
    let mut globals_written = false;
    let mut exported = false;
    let mut code = CodeSection::new();
    let mut bodies = 0;
    for payload in Parser::new(0).parse_all(file) {
        match payload.map_err(invalid)? {
            Payload::Version { .. } | Payload::End(_) | Payload::CustomSection(_) => {}
            Payload::TypeSection(reader) => copy(&mut out, SectionId::Type, file, reader.range()),
            Payload::ImportSection(reader) => {
                for import in reader.clone() {
                    let import = import.map_err(invalid)?;
                    let named = [import.module, import.name];
                    let function = matches!(import.ty, TypeRef::Func(_));
                    if !(named == TONE && function && has_type(&types, imported, TONE_SIGNATURE)) {
                        return Err(Refusal::Import(format!(
                            "{}.{}",
                            import.module, import.name
                        )));
                    }
                    imported += 1;
                }
                copy(&mut out, SectionId::Import, file, reader.range());
            }
            Payload::FunctionSection(reader) => {
                copy(&mut out, SectionId::Function, file, reader.range())
            }
            Payload::TableSection(reader) => copy(&mut out, SectionId::Table, file, reader.range()),
            Payload::MemorySection(_) => {
                // Capped where the contract says, whatever the file declares:
                // asked for past it, `memory.grow` fails, and the meter traps.
                let maximum = Some(memory.maximum.unwrap_or(pages).min(pages));
                let capped = MemoryType {
                    minimum: memory.initial,
                    maximum,
                    memory64: false,
                    shared: false,
                    page_size_log2: None,
                };
                out.section(MemorySection::new().memory(capped));
            }
            Payload::GlobalSection(reader) => {
                let mut globals = GlobalSection::new();
                let starts: Vec<usize> = reader
                    .clone()
                    .into_iter_with_offsets()
                    .map(|global| global.map(|(at, _)| at))
                    .collect::<Result<_, _>>()
                    .map_err(invalid)?;
                for (i, &start) in starts.iter().enumerate() {
                    globals.raw(
                        &file[start..starts.get(i + 1).copied().unwrap_or(reader.range().end)],
                    );
                }
                out.section(add_counters(&mut globals));
                globals_written = true;
            }
            Payload::ExportSection(reader) => {
                if !globals_written {
                    out.section(add_counters(&mut GlobalSection::new()));
                    globals_written = true;
                }
                let mut exports = ExportSection::new();
                let found: Vec<_> = reader
                    .into_iter()
                    .collect::<Result<_, _>>()
                    .map_err(invalid)?;
                let kept = |export: Export| {
                    found
                        .iter()
                        .find(|found| {
                            found.name == export.name() && found.kind == ExternalKind::Func
                        })
                        .map(|found| found.index)
                        .filter(|&index| has_type(&types, index, export.signature()))
                };
                for export in Export::REQUIRED {
                    let index = kept(export).ok_or(Refusal::Export(export.name()))?;
                    exports.export(export.name(), ExportKind::Func, index);
                }
                // One of another type is dropped, as a name outside the
                // contract is: the file runs as if it had none.
                for export in Export::OPTIONAL {
                    if let Some(index) = kept(export) {
                        exports.export(export.name(), ExportKind::Func, index);
                    }
                }
                found
                    .iter()
                    .find(|export| export.name == MEMORY && export.kind == ExternalKind::Memory)
                    .ok_or(Refusal::Export(MEMORY))?;
                exports.export(MEMORY, ExportKind::Memory, 0);
                exports.export(METER, ExportKind::Global, counts.meter);
                if let Some(start) = survey.start {
                    exports.export(START, ExportKind::Func, start);
                }
                out.section(&exports);
                exported = true;
            }
            // Exported instead, for the runner to call.
            Payload::StartSection { .. } => {}
            Payload::ElementSection(reader) => {
                elements_fit(reader.clone(), table)?;
                copy(&mut out, SectionId::Element, file, reader.range())
            }
            Payload::DataCountSection { range, .. } => {
                copy(&mut out, SectionId::DataCount, file, range)
            }
            Payload::CodeSectionStart { .. } => {}
            Payload::CodeSectionEntry(body) => {
                code.raw(&metered(file, &body, imported + bodies, &counts)?);
                bodies += 1;
                if bodies == types.function_count() - imported {
                    out.section(&code);
                }
            }
            Payload::DataSection(reader) => {
                data_fits(reader.clone(), memory.initial * PAGE)?;
                copy(&mut out, SectionId::Data, file, reader.range())
            }
            _ => {
                return Err(Refusal::Invalid(
                    "a section outside a core module".to_owned(),
                ));
            }
        }
    }
    if !exported {
        return Err(Refusal::Export(Export::CartAbi.name()));
    }
    Ok(Prepared {
        bytes: out.finish(),
    })
}

/// What validating a file learns of it.
struct Survey {
    types: Types,
    /// The values each function's frame holds, imported ones first: an
    /// import's frame is its parameters.
    frames: Vec<u32>,
    start: Option<u32>,
}

/// Validates `file`, each function's body measured on the way.
fn survey(file: &[u8]) -> Result<Survey, Refusal> {
    let mut validator = Validator::new_with_features(FEATURES);
    let mut allocations = FuncValidatorAllocations::default();
    let mut frames = Vec::new();
    let mut start = None;
    for payload in Parser::new(0).parse_all(file) {
        let payload = payload.map_err(invalid)?;
        match validator.payload(&payload).map_err(invalid)? {
            ValidPayload::Ok => {}
            ValidPayload::Func(func, body) => {
                let mut func = func.into_validator(std::mem::take(&mut allocations));
                frames.push(frame(&mut func, &body)?);
                allocations = func.into_allocations();
            }
            ValidPayload::End(types) => {
                let all = {
                    let types = types.as_ref();
                    let imported = types.function_count() - frames.len() as u32;
                    (0..imported)
                        .map(|function| function_type(&types, function).params().len() as u32)
                        .chain(frames)
                        .collect()
                };
                return Ok(Survey {
                    types,
                    frames: all,
                    start,
                });
            }
            ValidPayload::Parser(_) => {
                return Err(Refusal::Invalid("a module within a module".to_owned()));
            }
        }
        if let Payload::StartSection { func, .. } = payload {
            start = Some(func);
        }
    }
    Err(Refusal::Invalid("it ends early".to_owned()))
}

/// Validates a function's body and gives the values its frame holds: its
/// parameters, its locals and the deepest its operand stack goes.
fn frame(
    func: &mut FuncValidator<ValidatorResources>,
    body: &FunctionBody,
) -> Result<u32, Refusal> {
    let mut reader = body.get_binary_reader();
    func.read_locals(&mut reader).map_err(invalid)?;
    reader.set_features(*func.features());
    let mut deepest = 0;
    while !reader.eof() {
        let at = reader.original_position();
        let operator = reader.read_operator().map_err(invalid)?;
        func.op(at, &operator).map_err(invalid)?;
        deepest = deepest.max(func.operand_stack_height());
    }
    func.finish(reader.original_position()).map_err(invalid)?;
    let values = func.len_locals().saturating_add(deepest);
    if values > MAX_FRAME {
        return Err(Refusal::Frame(values));
    }
    Ok(values)
}

fn invalid(error: wasmparser::BinaryReaderError) -> Refusal {
    Refusal::Invalid(error.message().to_owned())
}

fn copy(out: &mut Module, id: SectionId, file: &[u8], range: Range<usize>) {
    out.section(&RawSection {
        id: id as u8,
        data: &file[range],
    });
}

fn function_type<'a>(types: &'a TypesRef, function: u32) -> &'a FuncType {
    types[types.core_function_at(function)].unwrap_func()
}

/// Whether `function` has the contract's `signature`, all `i32`.
fn has_type(types: &TypesRef, function: u32, signature: Signature) -> bool {
    let i32s = |list: &[wasmparser::ValType], n: usize| {
        list.len() == n && list.iter().all(|t| *t == wasmparser::ValType::I32)
    };
    let found = function_type(types, function);
    i32s(found.params(), signature.params) && i32s(found.results(), signature.results)
}

/// After a module's own globals: the meter, holding [`START_BUDGET`], then
/// the stack, holding nothing.
fn add_counters(globals: &mut GlobalSection) -> &GlobalSection {
    let ty = GlobalType {
        val_type: ValType::I32,
        mutable: true,
        shared: false,
    };
    globals.global(ty, &ConstExpr::i32_const(START_BUDGET as i32));
    globals.global(ty, &ConstExpr::i32_const(0))
}

/// A segment's offset. Importing no global, a module can only write it as
/// an `i32.const`.
fn offset(expr: &wasmparser::ConstExpr) -> Option<u64> {
    let mut operators = expr.get_operators_reader();
    match (operators.read().ok()?, operators.read().ok()?) {
        (Operator::I32Const { value }, Operator::End) => Some(u64::from(value as u32)),
        _ => None,
    }
}

/// Refuses an active data segment past the memory's first `bytes`: an
/// engine would refuse to instantiate it, each in its own words.
fn data_fits(reader: DataSectionReader, bytes: u64) -> Result<(), Refusal> {
    for data in reader {
        let data = data.map_err(invalid)?;
        if let DataKind::Active { offset_expr, .. } = data.kind {
            let end = offset(&offset_expr).map(|at| at + data.data.len() as u64);
            if end.is_none_or(|end| end > bytes) {
                return Err(Refusal::Segment);
            }
        }
    }
    Ok(())
}

/// Refuses an active element segment past the table's `size`.
fn elements_fit(reader: ElementSectionReader, size: u64) -> Result<(), Refusal> {
    for element in reader {
        let element = element.map_err(invalid)?;
        if let ElementKind::Active { offset_expr, .. } = element.kind {
            let count = match element.items {
                ElementItems::Functions(items) => items.count(),
                ElementItems::Expressions(_, items) => items.count(),
            };
            let end = offset(&offset_expr).map(|at| at + u64::from(count));
            if end.is_none_or(|end| end > size) {
                return Err(Refusal::Segment);
            }
        }
    }
    Ok(())
}

/// What the meter and the stack's count need of the module.
struct Counts<'a> {
    meter: u32,
    stack: u32,
    types: TypesRef<'a>,
    frames: &'a [u32],
    /// The largest frame of a function of each type.
    largest: HashMap<FuncType, u32>,
}

impl<'a> Counts<'a> {
    fn new(types: TypesRef<'a>, frames: &'a [u32]) -> Counts<'a> {
        let mut largest: HashMap<FuncType, u32> = HashMap::new();
        for (function, &frame) in (0..).zip(frames) {
            let largest = largest
                .entry(function_type(&types, function).clone())
                .or_default();
            *largest = (*largest).max(frame);
        }
        Counts {
            meter: types.global_count(),
            stack: types.global_count() + 1,
            types,
            frames,
            largest,
        }
    }

    /// What a call of `function` holds on the stack.
    fn direct(&self, function: u32) -> u32 {
        self.frames[function as usize] + FRAME_OVERHEAD
    }

    /// What a call through the table of a function of type `ty` holds.
    fn indirect(&self, ty: u32) -> u32 {
        let ty = self.types[self.types.core_type_at_in_module(ty)].unwrap_func();
        self.largest.get(ty).copied().unwrap_or(0) + FRAME_OVERHEAD
    }
}

/// What the meter and the stack's count do at an instruction.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Begins a loop: its turns are charged after it.
    Loop,
    /// Begins a `block` or an `if`.
    Opens,
    End,
    /// Moves or fills as many bytes as the top of the stack says.
    Bulk,
    /// Grows memory, and traps if it cannot.
    Grows,
    /// Calls, holding this much of the stack until it returns.
    Calls(u32),
    Plain,
}

/// Function `function`'s body with the meter and the stack's count written
/// in, as a code section holds it.
fn metered(
    file: &[u8],
    body: &FunctionBody,
    function: u32,
    counts: &Counts,
) -> Result<Vec<u8>, Refusal> {
    // The locals as declared, and one more: the scratch the meter reads a
    // length, a grown size or the stack into.
    let mut locals = body.get_locals_reader().map_err(invalid)?;
    let groups = locals.get_count();
    let declared_from = locals.original_position();
    let params = function_type(&counts.types, function).params().len() as u32;
    let mut declared = 0;
    for _ in 0..groups {
        declared += locals.read().map_err(invalid)?.0;
    }
    let scratch = params + declared;
    let declared_bytes = &file[declared_from..locals.original_position()];

    let mut reader = body.get_operators_reader().map_err(invalid)?;
    let mut ops: Vec<(Kind, Range<usize>)> = Vec::new();
    while !reader.eof() {
        let start = reader.original_position();
        let kind = match reader.read().map_err(invalid)? {
            Operator::Loop { .. } => Kind::Loop,
            Operator::Block { .. } | Operator::If { .. } => Kind::Opens,
            Operator::End => Kind::End,
            Operator::MemoryFill { .. }
            | Operator::MemoryCopy { .. }
            | Operator::MemoryInit { .. } => Kind::Bulk,
            Operator::MemoryGrow { .. } => Kind::Grows,
            Operator::Call { function_index } => Kind::Calls(counts.direct(function_index)),
            Operator::CallIndirect { type_index, .. } => Kind::Calls(counts.indirect(type_index)),
            Operator::TableGrow { .. } => return Err(Refusal::Instruction("table.grow")),
            Operator::TableFill { .. } => return Err(Refusal::Instruction("table.fill")),
            Operator::TableCopy { .. } => return Err(Refusal::Instruction("table.copy")),
            Operator::TableInit { .. } => return Err(Refusal::Instruction("table.init")),
            _ => Kind::Plain,
        };
        ops.push((kind, start..reader.original_position()));
    }

    // What is charged before each instruction: the function's locals and
    // count before the first, a loop's count after its `loop`. An
    // instruction counts in the innermost region around it, the function or
    // a loop, its `end` with it. A frame is at most MAX_FRAME values and a
    // body at most MAX_FILE bytes, so no charge nears i32::MAX.
    let mut charge = vec![0_i32; ops.len()];
    charge[0] = declared as i32;
    let mut regions = vec![0];
    let mut loops = Vec::new();
    for (i, (kind, _)) in ops.iter().enumerate() {
        charge[*regions.last().expect("the function's region stays")] += 1;
        match kind {
            Kind::Loop => {
                loops.push(true);
                regions.push(i + 1);
            }
            Kind::Opens => loops.push(false),
            Kind::End if loops.pop() == Some(true) => {
                regions.pop();
            }
            _ => {}
        }
    }

    let mut out = Vec::with_capacity(body.range().len() * 2);
    (groups + 1).encode(&mut out);
    out.extend_from_slice(declared_bytes);
    1u32.encode(&mut out);
    ValType::I32.encode(&mut out);
    let (meter, stack) = (counts.meter, counts.stack);
    let shift = BULK_BYTES.trailing_zeros() as i32;
    for ((kind, bytes), units) in ops.into_iter().zip(charge) {
        if units > 0 {
            emit(
                &mut out,
                &[
                    Instruction::GlobalGet(meter),
                    Instruction::I32Const(units),
                    Instruction::I32Sub,
                    Instruction::GlobalSet(meter),
                ],
            );
            trap_if_spent(&mut out, meter);
        }
        match kind {
            Kind::Bulk => {
                emit(
                    &mut out,
                    &[
                        Instruction::LocalTee(scratch),
                        Instruction::GlobalGet(meter),
                        Instruction::LocalGet(scratch),
                        Instruction::I32Const(shift),
                        Instruction::I32ShrU,
                        Instruction::I32Sub,
                        Instruction::GlobalSet(meter),
                    ],
                );
                trap_if_spent(&mut out, meter);
            }
            Kind::Calls(held) => emit(
                &mut out,
                &[
                    Instruction::GlobalGet(stack),
                    Instruction::I32Const(held as i32),
                    Instruction::I32Add,
                    Instruction::LocalTee(scratch),
                    Instruction::GlobalSet(stack),
                    Instruction::LocalGet(scratch),
                    Instruction::I32Const(MAX_STACK as i32),
                    Instruction::I32GtU,
                    Instruction::If(BlockType::Empty),
                    Instruction::I32Const(TOO_DEEP),
                    Instruction::GlobalSet(meter),
                    Instruction::Unreachable,
                    Instruction::End,
                ],
            ),
            _ => {}
        }
        out.extend_from_slice(&file[bytes]);
        match kind {
            Kind::Grows => emit(
                &mut out,
                &[
                    Instruction::LocalTee(scratch),
                    Instruction::I32Const(-1),
                    Instruction::I32Eq,
                    Instruction::If(BlockType::Empty),
                    Instruction::I32Const(OUT_OF_MEMORY),
                    Instruction::GlobalSet(meter),
                    Instruction::Unreachable,
                    Instruction::End,
                    Instruction::LocalGet(scratch),
                ],
            ),
            Kind::Calls(held) => emit(
                &mut out,
                &[
                    Instruction::GlobalGet(stack),
                    Instruction::I32Const(held as i32),
                    Instruction::I32Sub,
                    Instruction::GlobalSet(stack),
                ],
            ),
            _ => {}
        }
    }
    Ok(out)
}

fn trap_if_spent(out: &mut Vec<u8>, meter: u32) {
    emit(
        out,
        &[
            Instruction::GlobalGet(meter),
            Instruction::I32Const(0),
            Instruction::I32LtS,
            Instruction::If(BlockType::Empty),
            Instruction::Unreachable,
            Instruction::End,
        ],
    );
}

fn emit(out: &mut Vec<u8>, instructions: &[Instruction]) {
    for instruction in instructions {
        instruction.encode(out);
    }
}
