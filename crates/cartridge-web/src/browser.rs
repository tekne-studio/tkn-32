//! The browser's own WebAssembly behind [`Instance`]: V8 in Chrome,
//! JavaScriptCore in Safari, SpiderMonkey in Firefox. It runs the prepared
//! bytes as they are: the meter and the count of the stack are in the code,
//! so a cartridge stops here at the update it stops at on wasmi.

use std::cell::RefCell;
use std::rc::Rc;

use cartridge::{Export, MEMORY, TONE};
use cartridge_run::{Cart, Instance, METER, Prepared, START, Stop, Tone, Tones, Trapped, prepare};
use js_sys::WebAssembly::{Global, Memory, Module};
use js_sys::{ArrayBuffer, Function, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;

/// What `env.tone` is handed as: the seven numbers the contract names.
type ToneCall = dyn FnMut(i32, i32, i32, i32, i32, i32, i32);

/// How many functions the contract names a cartridge's exports by.
const EXPORTS: usize = Export::REQUIRED.len() + Export::OPTIONAL.len();

/// A prepared cartridge instantiated on the browser's WebAssembly.
pub struct Browser {
    /// The contract's exports, by [`Export`]'s order; an optional one the
    /// file lacks is none.
    exports: [Option<Function>; EXPORTS],
    start: Option<Function>,
    memory: Memory,
    meter: Global,
    tones: Rc<RefCell<Tones>>,
    /// The import the module calls: dropped with it, never before.
    _tone: Closure<ToneCall>,
}

impl Browser {
    /// Prepares `file`, instantiates it and starts it with `seed`: a cart
    /// running, or stopped with the screen saying why.
    pub fn open(file: &[u8], seed: u32) -> Cart<Browser> {
        match prepare(file) {
            Ok(prepared) => Browser::cart(&prepared, seed),
            Err(refusal) => Cart::stopped(Stop::Refused(refusal)),
        }
    }

    /// Instantiates a prepared cartridge and starts it with `seed`.
    pub fn cart(prepared: &Prepared, seed: u32) -> Cart<Browser> {
        match Browser::load(prepared) {
            Ok(instance) => Cart::start(instance, seed),
            Err(stop) => Cart::stopped(stop),
        }
    }

    /// Compiles and instantiates a prepared cartridge, at once: off the
    /// page's thread, where the browser allows it whatever the size.
    pub fn load(prepared: &Prepared) -> Result<Browser, Stop> {
        let module = Module::new(&Uint8Array::from(prepared.bytes())).map_err(load)?;
        let tones = Rc::new(RefCell::new(Tones::default()));
        let heard = Rc::clone(&tones);
        let tone = Closure::<ToneCall>::new(
            move |channel: i32,
                  hz: i32,
                  ms: i32,
                  wave: i32,
                  to_hz: i32,
                  loud: i32,
                  to_loud: i32| {
                let words = [channel, hz, ms, wave, to_hz, loud, to_loud].map(|word| word as u32);
                let [channel, hz, ms, wave, to_hz, loud, to_loud] = words;
                heard
                    .borrow_mut()
                    .hear(channel, hz, ms, wave, to_hz, loud, to_loud);
            },
        );
        let [module_name, name] = TONE;
        let env = Object::new();
        Reflect::set(&env, &name.into(), tone.as_ref()).map_err(load)?;
        let imports = Object::new();
        Reflect::set(&imports, &module_name.into(), &env).map_err(load)?;
        let instance = js_sys::WebAssembly::Instance::new(&module, &imports).map_err(load)?;
        let exports = instance.exports();
        let get = |name: &str| {
            Reflect::get(&exports, &name.into())
                .ok()
                .filter(|v| !v.is_undefined())
        };
        let missing = |name: &str| Stop::Load(format!("no {name} after preparing"));
        let function = |name: &str| {
            get(name)
                .and_then(|value| value.dyn_into::<Function>().ok())
                .ok_or_else(|| missing(name))
        };
        let mut functions: [Option<Function>; EXPORTS] = Default::default();
        for export in Export::REQUIRED {
            functions[export as usize] = Some(function(export.name())?);
        }
        for export in Export::OPTIONAL {
            functions[export as usize] = function(export.name()).ok();
        }
        Ok(Browser {
            exports: functions,
            start: get(START).and_then(|value| value.dyn_into().ok()),
            memory: get(MEMORY)
                .and_then(|value| value.dyn_into().ok())
                .ok_or_else(|| missing(MEMORY))?,
            meter: get(METER)
                .and_then(|value| value.dyn_into().ok())
                .ok_or_else(|| missing(METER))?,
            tones,
            _tone: tone,
        })
    }
}

/// The engine's words for why a module would not load.
fn load(error: JsValue) -> Stop {
    Stop::Load(match error.dyn_ref::<js_sys::Error>() {
        Some(error) => String::from(error.message()),
        None => format!("{error:?}"),
    })
}

impl Instance for Browser {
    fn start(&mut self) -> Result<(), Trapped> {
        match &self.start {
            Some(start) => start.call0(&JsValue::NULL).map(drop).map_err(|_| Trapped),
            None => Ok(()),
        }
    }

    fn call(&mut self, export: Export, arg: u32) -> Result<u32, Trapped> {
        let Some(function) = &self.exports[export as usize] else {
            return Ok(0);
        };
        let got = match export.signature().params {
            0 => function.call0(&JsValue::NULL),
            _ => function.call1(&JsValue::NULL, &JsValue::from(arg as i32)),
        }
        // A trap is a `RuntimeError`, a stack the engine ran out of a
        // `RangeError`: either stops the cartridge.
        .map_err(|_| Trapped)?;
        // An i32 comes back as a number within an i32's range.
        Ok(got.as_f64().map_or(0, |n| n as i32 as u32))
    }

    fn read(&self, at: u32, into: &mut [u8]) -> bool {
        // The memory may have grown since the last read: its buffer is
        // asked for each time.
        let buffer: ArrayBuffer = self.memory.buffer().unchecked_into();
        let end = u64::from(at) + into.len() as u64;
        if end > u64::from(buffer.byte_length()) {
            return false;
        }
        Uint8Array::new_with_byte_offset_and_length(&buffer, at, into.len() as u32).copy_to(into);
        true
    }

    fn meter(&self) -> i32 {
        self.meter.value().as_f64().map_or(0, |units| units as i32)
    }

    fn set_meter(&mut self, units: i32) {
        self.meter.set_value(&JsValue::from(units));
    }

    fn take_tones(&mut self) -> Vec<Tone> {
        self.tones.borrow_mut().take()
    }
}
