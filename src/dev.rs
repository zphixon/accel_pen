use notify::Watcher;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tera::Tera;

pub fn reload_task(tera: Arc<RwLock<Tera>>) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();

        watcher
            .watch(
                &PathBuf::from("frontend/templates"),
                notify::RecursiveMode::Recursive,
            )
            .unwrap();

        watcher
            .watch(
                &PathBuf::from("frontend/modules/src"),
                notify::RecursiveMode::Recursive,
            )
            .unwrap();

        while let Ok(event) = rx.recv() {
            match event {
                Ok(notify::Event {
                    kind: notify::EventKind::Modify(_),
                    mut paths,
                    ..
                }) => {
                    paths = paths
                        .into_iter()
                        .filter(|path| {
                            path.parent()
                                .unwrap()
                                .file_name()
                                .unwrap()
                                .to_string_lossy()
                                != "bindings"
                        })
                        .collect();

                    if paths.is_empty() {
                        continue;
                    }

                    tracing::debug!(
                        "Reloading templates: {}",
                        paths.iter().fold(String::new(), |acc, next| acc
                            + " "
                            + &next.display().to_string())
                    );

                    if paths
                        .iter()
                        .any(|path| path.ancestors().any(|unc| unc.ends_with("modules")))
                    {
                        let output = std::process::Command::new("./node_modules/.bin/rollup").arg("-c")
                            .current_dir("frontend/modules")
                            .output();
                        println!("tsc: {:?}", output);
                    }

                    match tera.write().unwrap().full_reload() {
                        Ok(_) => {}
                        Err(err) => {
                            use std::error::Error;
                            tracing::error!(
                                "Couldn't reload templates: {} {:?}",
                                err,
                                err.source()
                            );
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("template watcher error: {}", err)
                }
                _ => {}
            }
        }
    });
}
