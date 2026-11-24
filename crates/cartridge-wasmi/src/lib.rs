//! A cartridge's native engine: wasmi, a WebAssembly interpreter in plain
//! Rust, behind `cartridge-run`'s [`Instance`]. It runs the prepared bytes
//! with its own fuel off: the meter is in the code.

use cartridge::{Export, FRAME_OVERHEAD, MAX_FRAME, MAX_STACK, MEMORY, TONE};
use cartridge_run::{Cart, Instance, METER, Prepared, START, Stop, Tone, Tones, Trapped, prepare};
use wasmi::{
    Caller, CompilationMode, Config, Engine, Global, Linker, Memory, Module, Store, TypedFunc, Val,
};

/// The calls wasmi lets a chain hold: twice what the contract allows, the
/// runner's own call and `tone` beside, so the cartridge's count stops it
/// first.
const DEPTH: usize = 2 * (MAX_STACK / FRAME_OVERHEAD) as usize + 2;
/// The bytes wasmi lets its value stack take, eight a value: eight times
/// what the contract allows a chain and the runner's call.
const HEIGHT: usize = 8 * 8 * (MAX_STACK + MAX_FRAME) as usize;

/// A prepared cartridge instantiated on wasmi.
pub struct Wasmi {
    store: Store<Tones>,
    memory: Memory,
    meter: Global,
    start: Option<TypedFunc<(), ()>>,
    cart_abi: TypedFunc<(), i32>,
    init: TypedFunc<i32, ()>,
    update: TypedFunc<i32, ()>,
    frame: TypedFunc<(), i32>,
    palette: TypedFunc<(), i32>,
    screen: Option<TypedFunc<(), i32>>,
}

impl Wasmi {
    /// Prepares `file`, instantiates it and starts it with `seed`: a cart
    /// running, or stopped with the screen saying why.
    pub fn open(file: &[u8], seed: u32) -> Cart<Wasmi> {
        match prepare(file) {
            Ok(prepared) => Wasmi::cart(&prepared, seed),
            Err(refusal) => Cart::stopped(Stop::Refused(refusal)),
        }
    }

    /// Instantiates a prepared cartridge and starts it with `seed`.
    pub fn cart(prepared: &Prepared, seed: u32) -> Cart<Wasmi> {
        match Wasmi::load(prepared) {
            Ok(instance) => Cart::start(instance, seed),
            Err(stop) => Cart::stopped(stop),
        }
    }

    /// Compiles and instantiates a prepared cartridge, every function
    /// translated at once: a module wasmi cannot run stops here, never
    /// in the middle of a game.
    pub fn load(prepared: &Prepared) -> Result<Wasmi, Stop> {
        let load = |error: wasmi::Error| Stop::Load(error.to_string());
        let mut config = Config::default();
        config
            .compilation_mode(CompilationMode::Eager)
            .set_max_recursion_depth(DEPTH)
            .set_max_stack_height(HEIGHT);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, prepared.bytes()).map_err(load)?;
        let mut store = Store::new(&engine, Tones::default());
        let mut linker = <Linker<Tones>>::new(&engine);
        let [module_name, name] = TONE;
        linker
            .func_wrap(
                module_name,
                name,
                |mut caller: Caller<'_, Tones>,
                 channel: i32,
                 hz: i32,
                 ms: i32,
                 wave: i32,
                 to_hz: i32,
                 loud: i32,
                 to_loud: i32| {
                    let words =
                        [channel, hz, ms, wave, to_hz, loud, to_loud].map(|word| word as u32);
                    let [channel, hz, ms, wave, to_hz, loud, to_loud] = words;
                    caller
                        .data_mut()
                        .hear(channel, hz, ms, wave, to_hz, loud, to_loud);
                },
            )
            .map_err(|error| Stop::Load(error.to_string()))?;
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(load)?;
        let missing = |name: &str| Stop::Load(format!("no {name} after preparing"));
        let memory = instance
            .get_memory(&store, MEMORY)
            .ok_or_else(|| missing(MEMORY))?;
        let meter = instance
            .get_global(&store, METER)
            .ok_or_else(|| missing(METER))?;
        let start = instance.get_typed_func::<(), ()>(&store, START).ok();
        let ask = |export: Export| {
            instance
                .get_typed_func::<(), i32>(&store, export.name())
                .map_err(load)
        };
        let tell = |export: Export| {
            instance
                .get_typed_func::<i32, ()>(&store, export.name())
                .map_err(load)
        };
        Ok(Wasmi {
            cart_abi: ask(Export::CartAbi)?,
            init: tell(Export::Init)?,
            update: tell(Export::Update)?,
            frame: ask(Export::Frame)?,
            palette: ask(Export::Palette)?,
            screen: ask(Export::Screen).ok(),
            start,
            store,
            memory,
            meter,
        })
    }
}

impl Instance for Wasmi {
    fn start(&mut self) -> Result<(), Trapped> {
        match &self.start {
            Some(start) => start.call(&mut self.store, ()).map_err(|_| Trapped),
            None => Ok(()),
        }
    }

    fn call(&mut self, export: Export, arg: u32) -> Result<u32, Trapped> {
        let store = &mut self.store;
        match export {
            Export::CartAbi => self.cart_abi.call(store, ()).map(|got| got as u32),
            Export::Frame => self.frame.call(store, ()).map(|got| got as u32),
            Export::Palette => self.palette.call(store, ()).map(|got| got as u32),
            Export::Screen => match &self.screen {
                Some(screen) => screen.call(store, ()).map(|got| got as u32),
                None => Ok(0),
            },
            Export::Init => self.init.call(store, arg as i32).map(|()| 0),
            Export::Update => self.update.call(store, arg as i32).map(|()| 0),
        }
        .map_err(|_| Trapped)
    }

    fn read(&self, at: u32, into: &mut [u8]) -> bool {
        self.memory.read(&self.store, at as usize, into).is_ok()
    }

    fn meter(&self) -> i32 {
        match self.meter.get(&self.store) {
            Val::I32(units) => units,
            _ => 0,
        }
    }

    fn set_meter(&mut self, units: i32) {
        // The meter is the prepared file's own i32 global: setting it fails
        // only on a type the preparation never writes.
        let _ = self.meter.set(&mut self.store, Val::I32(units));
    }

    fn take_tones(&mut self) -> Vec<Tone> {
        self.store.data_mut().take()
    }
}
