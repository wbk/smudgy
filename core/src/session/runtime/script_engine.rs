use std::{
    cell::RefCell,
    collections::HashSet,
    fs,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use deno_core::{
    ascii_str_include, error::CoreError, v8::{self, script_compiler::Source, Global, Handle}, PollEventLoopOptions
};

use derive_more::{Display, Into};
use iced::futures::channel::mpsc::Sender;
use rustyscript::{ExtensionOptions, RustyResolver};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    get_smudgy_home,
    session::{
        SessionId, TaggedSessionEvent,
        runtime::{ActionResult, trigger::Manager},
    },
};

use anyhow::{Result, bail, anyhow, Context as AnyhowContext};
use deno_core::url::Url;

use super::RuntimeAction;

mod ops;

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, Hash, Into)]
pub struct ScriptId(usize);

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, Hash, Into)]
pub struct FunctionId(usize);

pub struct ScriptEngineParams<'a> {
    pub session_id: SessionId,
    pub server_name: &'a Arc<String>,
    pub profile_name: &'a Arc<String>,
    pub ui_tx: Sender<TaggedSessionEvent>,
    pub runtime_oob_tx: UnboundedSender<RuntimeAction>,
}

pub struct ScriptEngine<'a> {
    session_id: SessionId,
    rustyscript_runtime: rustyscript::Runtime,
    tokio_runtime: Rc<tokio::runtime::Runtime>,
    server_name: &'a Arc<String>,
    profile_name: &'a Arc<String>,
    ui_tx: Sender<TaggedSessionEvent>,
    runtime_oob_tx: UnboundedSender<RuntimeAction>,

    script_functions: Rc<RefCell<Vec<v8::Global<v8::Function>>>>,
    compiled_scripts: Vec<v8::Global<v8::Script>>,
}

impl<'a> Drop for ScriptEngine<'a> {
    fn drop(&mut self) {
        println!("Dropping script engine");
    }
}

impl<'a> ScriptEngine<'a> {
    /// Load all JavaScript and TypeScript modules from the profile's modules directory
    async fn load_modules(
        server_name: &str,
        script_engine: &mut rustyscript::Runtime,
    ) -> Result<()> {
        let smudgy_dir = get_smudgy_home()
            .context("Failed to get smudgy home directory")?;
        let mut modules_dir = smudgy_dir.join(server_name);
        modules_dir.push("modules");

        // Check if modules directory exists
        if !modules_dir.exists() {
            info!("Modules directory does not exist: {:?}", modules_dir);
            return Ok(());
        }

        // Collect module file paths as URLs
        let mut module_paths = Vec::new();
        let entries = fs::read_dir(&modules_dir)
            .with_context(|| format!("Could not read from modules directory: {:?}", modules_dir))?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let file_type = entry.file_type().context("Failed to get file type")?;
            
            if file_type.is_file() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_str()
                    .context("File name contains invalid UTF-8")?;
                
                // Check for .js or .ts extension
                if file_name_str.ends_with(".js") || file_name_str.ends_with(".ts") {
                    let file_path = entry.path();
                    
                    // Convert file path to URL
                    let url = Url::from_file_path(file_path)
                        .map_err(|_| anyhow!("Failed to convert file path to URL"))?;
                    
                    info!("Found module: {}", url);
                    module_paths.push(url.to_string());
                }
            }
        }

        // If no modules found, return early
        if module_paths.is_empty() {
            info!("No modules found in directory: {:?}", modules_dir);
            return Ok(());
        }

        // Generate import code for all modules
        let code = module_paths
            .iter()
            .map(|path| format!("import '{}';", path))
            .collect::<Vec<String>>()
            .join("\n");

        info!("Loading {} modules with code:\n{}", module_paths.len(), code);

        let deno = script_engine.deno_runtime();

        // Load the generated import code as an ES module
        match deno
            .load_main_es_module_from_code(
                &Url::parse("smudgy://modules").context("Failed to parse smudgy modules URL")?,
                code
            )
            .await
        {
            Ok(module_id) => {
                let evaluation_result = {
                    let mut receiver = deno.mod_evaluate(module_id);

                    tokio::select! {
                        biased;

                        maybe_result = &mut receiver => {
                            maybe_result
                        }

                        event_loop_result = deno.run_event_loop(PollEventLoopOptions::default()) => {
                            event_loop_result?;
                            receiver.await
                        }
                    }
                };

                if let Err(e) = evaluation_result {
                    warn!("Failed to evaluate modules: {:?}", e);
                } else {
                    info!("Successfully loaded {} modules", module_paths.len());
                }
            }
            Err(e) => {
                warn!("Failed to load modules: {:?}", e);
            }
        }

        Ok(())
    }

    fn create_runtime(&self) -> Result<rustyscript::Runtime> {
        let smudgy_dir = get_smudgy_home().unwrap();
        let server_path = smudgy_dir.join(self.server_name.as_str());

        let mut rustyscript_runtime = rustyscript::Runtime::with_tokio_runtime(
            rustyscript::RuntimeOptions {
                extensions: vec![ops::smudgy_ops::init(
                    self.session_id,
                    self.script_functions.clone(),
                    self.runtime_oob_tx.clone(),
                )],
                extension_options: ExtensionOptions {
                    webstorage_origin_storage_dir: Some(server_path.join("localstorage")),
                    node_resolver: Arc::new(RustyResolver::new(
                        Some(server_path),
                        Arc::new(rustyscript::extensions::deno_fs::RealFs),
                    )),

                    ..Default::default()
                },
                schema_whlist: HashSet::from(["smudgy".to_string()]),
                ..Default::default()
            },
            self.tokio_runtime.clone(),
        )
        .expect("Failed to create JS runtime");

        rustyscript_runtime.deno_runtime().execute_script("<smudgy>", ascii_str_include!("js/smudgy.js"))
        .unwrap();

        // Load modules from the modules directory
        self.tokio_runtime.block_on(async {
            if let Err(e) = Self::load_modules(self.server_name.as_str(), &mut rustyscript_runtime).await {
                warn!("Failed to load modules: {:?}", e);
            }
        });

        Ok(rustyscript_runtime)
   
    }

    pub fn new(params: ScriptEngineParams<'a>) -> Self {
        let tokio_runtime = Rc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create script engine tokio runtime"),
        );

        let smudgy_dir = get_smudgy_home().unwrap();
        let server_path = smudgy_dir.join(params.server_name.as_str());

        let script_functions = Rc::new(RefCell::new(Vec::new()));

        let mut rustyscript_runtime = rustyscript::Runtime::with_tokio_runtime(
            rustyscript::RuntimeOptions {
                extensions: vec![ops::smudgy_ops::init(
                    params.session_id,
                    script_functions.clone(),
                    params.runtime_oob_tx.clone(),
                )],
                extension_options: ExtensionOptions {
                    webstorage_origin_storage_dir: Some(server_path.join("localstorage")),
                    node_resolver: Arc::new(RustyResolver::new(
                        Some(server_path.clone()),
                        Arc::new(rustyscript::extensions::deno_fs::RealFs),
                    )),

                    ..Default::default()
                },
                schema_whlist: HashSet::from(["smudgy".to_string()]),
                ..Default::default()
            },
            tokio_runtime.clone(),
        )
        .expect("Failed to create JS runtime");

        rustyscript_runtime.deno_runtime().execute_script("<smudgy>", ascii_str_include!("js/smudgy.js"))
        .unwrap();

        // Load modules from the modules directory
        tokio_runtime.block_on(async {
            if let Err(e) = Self::load_modules(params.server_name.as_str(), &mut rustyscript_runtime).await {
                warn!("Failed to load modules: {:?}", e);
            }
        });

        
        Self {
            session_id: params.session_id,
            tokio_runtime,
            rustyscript_runtime,
            server_name: params.server_name,
            profile_name: params.profile_name,
            ui_tx: params.ui_tx,
            runtime_oob_tx: params.runtime_oob_tx,
            script_functions,
            compiled_scripts: Vec::new(),
        }
    }

    pub fn tick_interval() -> tokio::time::Interval {
        let mut tick_interval = tokio::time::interval(tokio::time::Duration::from_micros(100));
        tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tick_interval
    }

    pub fn poll_event_loop(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), CoreError>> {
        self.rustyscript_runtime
            .deno_runtime()
            .poll_event_loop(cx, PollEventLoopOptions::default())
    }

    pub fn deno_runtime(&mut self) -> &mut deno_core::JsRuntime {
        self.rustyscript_runtime.deno_runtime()
    }

    pub fn reload(&mut self) -> Result<()> {
        // Create completely new containers and runtime first
        let new_script_functions = Rc::new(RefCell::new(Vec::new()));
        let new_compiled_scripts = Vec::new();
        
        // Temporarily swap the script_functions so create_runtime uses the new one
        let old_script_functions = std::mem::replace(&mut self.script_functions, new_script_functions);
        
        // Create the new runtime with fresh containers
        let new_runtime = self.create_runtime()?;
        
        // Now replace everything at once - this should drop the old runtime cleanly
        self.rustyscript_runtime = new_runtime;
        self.compiled_scripts = new_compiled_scripts;
        // script_functions is already replaced above
        
        // Let the old_script_functions drop naturally here
        drop(old_script_functions);
        
        println!("Reloaded script engine");
        Ok(())
    }

    #[inline]
    pub fn call_javascript_function(
        &mut self,
        trigger_manager: &Manager,
        function_id: FunctionId,
        matches: &Arc<Vec<(String, String)>>,
        depth: u32,
    ) -> Result<ActionResult> {
        let started = Instant::now();

        let script_functions = self.script_functions.clone();

        if let Some(f) = script_functions.borrow().get(usize::from(function_id)) {
            let mut scope = self.deno_runtime().handle_scope();

            let result = {
                let try_catch = &mut v8::TryCatch::new(&mut scope);

                let matches_object = v8::Object::new(try_catch);
                for (k, v) in matches.iter() {
                    let arg_k = v8::String::new(try_catch, k).unwrap();
                    let arg_v = v8::String::new(try_catch, v).unwrap();
                    matches_object.create_data_property(try_catch, arg_k.into(), arg_v.into());
                }

                let f = f.open(try_catch);
                let f_this = v8::undefined(try_catch).into();
                let result = f.call(try_catch, f_this, &[matches_object.into()]);

                if try_catch.has_caught() {
                    let ex = try_catch.exception().unwrap();
                    let exc = ex.to_string(try_catch).unwrap();
                    let exc = exc.to_rust_string_lossy(try_catch);
                    Ok(ActionResult::Echo(exc))
                } else if let Some(value) = result {
                    if value.boolean_value(try_catch) {
                        let output = value.open(try_catch).to_rust_string_lossy(try_catch);
                        trigger_manager.process_nested_outgoing_line(output.as_str(), depth + 1)?;
                        Ok(ActionResult::None)
                    } else {
                        Ok(ActionResult::None)
                    }
                } else {
                    Ok(ActionResult::None)
                }
            };

            let elapsed = started.elapsed();
            debug!(
                "Script execution on {} took {:?}",
                matches
                    .first()
                    .unwrap_or(&(String::new(), "unknown".to_string()))
                    .1,
                elapsed
            );

            return result;
        } else {
            bail!("Function {} not found", function_id)
        }
    }

    #[inline]
    pub fn run_script(
        &mut self,
        trigger_manager: &Manager,
        script_id: ScriptId,
        matches: &Arc<Vec<(String, String)>>,
        depth: u32,
    ) -> Result<ActionResult> {
        let started = Instant::now();

        // Get the script before creating the mutable scope to avoid borrowing conflicts
        let script = self
            .compiled_scripts
            .get(usize::from(script_id))
            .ok_or_else(|| anyhow::anyhow!("Script {} not found", script_id))?
            .clone();

        let mut scope = self.deno_runtime().handle_scope();
        let result = {
            let try_catch = &mut v8::TryCatch::new(&mut scope);

            let matches_object = v8::Object::new(try_catch);
            for (k, v) in matches.iter() {
                let arg_k = v8::String::new(try_catch, k).unwrap();
                let arg_v = v8::String::new(try_catch, v).unwrap();
                matches_object.create_data_property(try_catch, arg_k.into(), arg_v.into());
            }

            let matches_name = v8::String::new(try_catch, "matches").unwrap();

            try_catch.get_current_context().global(try_catch).set(
                try_catch,
                matches_name.into(),
                matches_object.into(),
            );

            let result = script.open(try_catch).run(try_catch);

            if try_catch.has_caught() {
                let ex = try_catch.exception().unwrap();
                let exc = ex.to_string(try_catch).unwrap();
                let exc = exc.to_rust_string_lossy(try_catch);
                Ok(ActionResult::Echo(exc))
            } else if let Some(value) = result {
                if value.boolean_value(try_catch) {
                    let output = value.open(try_catch).to_rust_string_lossy(try_catch);
                    trigger_manager.process_nested_outgoing_line(output.as_str(), depth + 1)?;
                    Ok(ActionResult::None)
                } else {
                    Ok(ActionResult::None)
                }
            } else {
                Ok(ActionResult::None)
            }
        };

        let elapsed = started.elapsed();
        info!(
            "Script execution on {} took {:?}",
            matches
                .first()
                .unwrap_or(&(String::new(), "unknown".to_string()))
                .1,
            elapsed
        );
        result
    }

    pub fn add_script(&mut self, source: &str) -> Result<ScriptId> {
        let script = compile_javascript(&mut self.deno_runtime().handle_scope(), source)?;
        let script_id = ScriptId(self.compiled_scripts.len());
        self.compiled_scripts.push(script);
        Ok(script_id)
    }
}

fn compile_javascript(scope: &mut v8::HandleScope, source: &str) -> Result<v8::Global<v8::Script>> {
    let v8_script_source =
        v8::String::new_from_utf8(scope, source.as_bytes(), v8::NewStringType::Normal).unwrap();

    if let Some(unbound_script) = v8::script_compiler::compile_unbound_script(
        scope,
        &mut Source::new(v8_script_source, None),
        v8::script_compiler::CompileOptions::NoCompileOptions,
        v8::script_compiler::NoCacheReason::BecauseV8Extension,
    ) {
        let bound_script = unbound_script.open(scope).bind_to_current_context(scope);

        Ok(Global::new(scope, bound_script))
    } else {
        Err(anyhow!("Failed to compile script"))
    }
}
