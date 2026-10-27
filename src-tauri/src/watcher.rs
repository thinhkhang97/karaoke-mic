//! The audio manager thread. It owns the running `Engine` (cpal streams are
//! not `Send`), answers requests from Tauri commands, and every poll interval
//! re-checks the device lists so a plugged or unplugged device is picked up.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use cpal::Host;

use crate::commands::{AudioStatus, StatusParts, build_status};
use crate::devices::{choose_input, choose_output};
use crate::dsp::Params;
use crate::engine::{Engine, EngineShared, list_inputs, list_outputs};
use crate::params::{LevelMeter, SharedParams};
use crate::platform::{MicPermission, is_bluetooth_output, mic_permission};

const POLL_INTERVAL: Duration = Duration::from_millis(1500);
const PERMISSION_MESSAGE: &str = "Microphone access is off. Turn it on for Karaoke Mic in System Settings → Privacy & Security → Microphone.";

enum Request {
    Select { input: Option<String>, output: Option<String>, reply: Sender<AudioStatus> },
    Status { reply: Sender<AudioStatus> },
}

/// Handle kept in Tauri state; cheap to clone into blocking tasks.
#[derive(Clone)]
pub struct AudioHandle {
    tx: Sender<Request>,
    pub params: Arc<SharedParams>,
    pub meter: Arc<LevelMeter>,
}

impl AudioHandle {
    /// Sets the preferred devices and (re)starts audio with them.
    pub fn select(&self, input: Option<String>, output: Option<String>) -> Result<AudioStatus, String> {
        self.ask(|reply| Request::Select { input, output, reply })
    }

    pub fn status(&self) -> Result<AudioStatus, String> {
        self.ask(|reply| Request::Status { reply })
    }

    fn ask(&self, make: impl FnOnce(Sender<AudioStatus>) -> Request) -> Result<AudioStatus, String> {
        let (reply, rx) = mpsc::channel();
        self.tx.send(make(reply)).map_err(|_| "The audio engine has stopped".to_string())?;
        rx.recv().map_err(|_| "The audio engine is not responding".to_string())
    }
}

/// Starts the manager thread. `emit` is called whenever the status changes
/// on its own (devices plugged or unplugged, a stream failing).
pub fn spawn(emit: impl Fn(&AudioStatus) + Send + 'static) -> AudioHandle {
    let shared = EngineShared {
        params: Arc::new(SharedParams::new(Params::default())),
        meter: Arc::new(LevelMeter::default()),
        failed: Arc::new(AtomicBool::new(false)),
    };
    let (tx, rx) = mpsc::channel();
    let handle = AudioHandle { tx, params: shared.params.clone(), meter: shared.meter.clone() };
    thread::spawn(move || Manager::new(shared).run(rx, emit));
    handle
}

struct Manager {
    host: Host,
    shared: EngineShared,
    engine: Option<Engine>,
    preferred_input: Option<String>,
    preferred_output: Option<String>,
    started: bool,
    error: Option<String>,
    last_status: Option<AudioStatus>,
}

impl Manager {
    fn new(shared: EngineShared) -> Manager {
        Manager {
            host: cpal::default_host(),
            shared,
            engine: None,
            preferred_input: None,
            preferred_output: None,
            started: false,
            error: None,
            last_status: None,
        }
    }

    fn run(mut self, rx: Receiver<Request>, emit: impl Fn(&AudioStatus)) {
        loop {
            match rx.recv_timeout(POLL_INTERVAL) {
                Ok(Request::Select { input, output, reply }) => {
                    self.preferred_input = input;
                    self.preferred_output = output;
                    self.started = true;
                    let status = self.reconcile(true);
                    self.last_status = Some(status.clone());
                    let _ = reply.send(status);
                }
                Ok(Request::Status { reply }) => {
                    let _ = reply.send(self.reconcile(false));
                }
                Err(RecvTimeoutError::Timeout) => {
                    if !self.started {
                        continue;
                    }
                    let status = self.reconcile(false);
                    if self.last_status.as_ref() != Some(&status) {
                        emit(&status);
                        self.last_status = Some(status);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Makes the running engine match the devices that should be in use and
    /// returns the resulting status. `force` restarts even if nothing changed.
    fn reconcile(&mut self, force: bool) -> AudioStatus {
        let inputs = list_inputs(&self.host);
        let outputs = list_outputs(&self.host);
        let want_in = choose_input(&inputs, self.preferred_input.as_deref());
        let want_out = choose_output(&outputs, self.preferred_output.as_deref());

        let failed = self.shared.failed.swap(false, Ordering::Relaxed);
        let matches = self.engine.as_ref().is_some_and(|e| {
            Some(&e.input) == want_in.as_ref() && Some(&e.output) == want_out.as_ref()
        });
        if self.started && (force || failed || !matches) {
            self.restart(want_in.as_deref(), want_out.as_deref());
        }

        let permission = mic_permission();
        let error = match permission {
            MicPermission::Denied => Some(PERMISSION_MESSAGE.to_string()),
            _ => self.error.clone(),
        };
        let active_in = self.engine.as_ref().map(|e| e.input.as_str());
        let active_out = self.engine.as_ref().map(|e| e.output.as_str());
        build_status(StatusParts {
            inputs: &inputs,
            outputs: &outputs,
            active_input: active_in,
            active_output: active_out,
            output_is_bluetooth: active_out.is_some_and(is_bluetooth_output),
            mic_permission: permission,
            error,
        })
    }

    fn restart(&mut self, input: Option<&str>, output: Option<&str>) {
        // Release the old devices before opening new ones.
        self.engine = None;
        let (Some(input), Some(output)) = (input, output) else {
            self.error = Some("No microphone or speaker found".to_string());
            return;
        };
        match Engine::start(&self.host, input, output, &self.shared) {
            Ok(engine) => {
                self.engine = Some(engine);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Could not open the audio device: {e}")),
        }
    }
}
