// SPDX-License-Identifier: GPL-3.0-or-later

//! Secret Service access through `oo7`, driven on a dedicated worker thread so
//! the GTK main loop never runs an async runtime or blocks.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

use futures_channel::oneshot;
use zeroize::Zeroizing;

/// The `xdg:schema` attribute shared by every Quaderno keyring item.
pub const SCHEMA: &str = "io.github.stickgrinder.Quaderno.Vault";

/// The attributes that identify a vault's keyring entry.
pub fn attributes(vault_path: &str) -> HashMap<String, String> {
    HashMap::from([
        ("xdg:schema".to_owned(), SCHEMA.to_owned()),
        ("vault-path".to_owned(), vault_path.to_owned()),
    ])
}

/// A keyring operation failure.
#[derive(Debug, Clone)]
pub enum KeyringError {
    /// No Secret Service is available on this system.
    Unavailable,
    /// The backend failed.
    Failure(String),
}

impl std::fmt::Display for KeyringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyringError::Unavailable => write!(formatter, "no secret service is available"),
            KeyringError::Failure(message) => write!(formatter, "{message}"),
        }
    }
}

type Lookup = Result<Option<Zeroizing<Vec<u8>>>, KeyringError>;
type Unit = Result<(), KeyringError>;

enum Job {
    Available(oneshot::Sender<bool>),
    Lookup {
        path: String,
        reply: oneshot::Sender<Lookup>,
    },
    Store {
        path: String,
        passphrase: Zeroizing<Vec<u8>>,
        reply: oneshot::Sender<Unit>,
    },
    Delete {
        path: String,
        reply: oneshot::Sender<Unit>,
    },
}

/// A handle to the keyring worker thread. Cloning shares the same worker.
#[derive(Clone)]
pub struct Keyring {
    sender: mpsc::Sender<Job>,
}

impl Keyring {
    /// Starts the worker thread.
    pub fn spawn() -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("quaderno-keyring".to_owned())
            .spawn(move || run(receiver))
            .expect("spawning the keyring worker thread");
        Self { sender }
    }

    /// Asks whether a Secret Service is available.
    pub fn available(&self) -> oneshot::Receiver<bool> {
        let (reply, receiver) = oneshot::channel();
        let _ = self.sender.send(Job::Available(reply));
        receiver
    }

    /// Looks up a stored passphrase for `path`.
    pub fn lookup(&self, path: String) -> oneshot::Receiver<Lookup> {
        let (reply, receiver) = oneshot::channel();
        let _ = self.sender.send(Job::Lookup { path, reply });
        receiver
    }

    /// Stores (or replaces) the passphrase for `path`.
    pub fn store(&self, path: String, passphrase: Zeroizing<Vec<u8>>) -> oneshot::Receiver<Unit> {
        let (reply, receiver) = oneshot::channel();
        let _ = self.sender.send(Job::Store {
            path,
            passphrase,
            reply,
        });
        receiver
    }

    /// Removes the stored passphrase for `path`.
    pub fn delete(&self, path: String) -> oneshot::Receiver<Unit> {
        let (reply, receiver) = oneshot::channel();
        let _ = self.sender.send(Job::Delete { path, reply });
        receiver
    }
}

fn run(receiver: mpsc::Receiver<Job>) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            for job in receiver {
                handle(job, None, None);
            }
            return;
        }
    };

    let keyring = runtime.block_on(async { oo7::Keyring::new().await.ok() });
    for job in receiver {
        handle(job, keyring.as_ref(), Some(&runtime));
    }
}

fn handle(job: Job, keyring: Option<&oo7::Keyring>, runtime: Option<&tokio::runtime::Runtime>) {
    match job {
        Job::Available(reply) => {
            let _ = reply.send(keyring.is_some());
        }
        Job::Lookup { path, reply } => {
            let result = match (keyring, runtime) {
                (Some(keyring), Some(runtime)) => lookup(runtime, keyring, &path),
                _ => Err(KeyringError::Unavailable),
            };
            let _ = reply.send(result);
        }
        Job::Store {
            path,
            passphrase,
            reply,
        } => {
            let result = match (keyring, runtime) {
                (Some(keyring), Some(runtime)) => store(runtime, keyring, &path, &passphrase),
                _ => Err(KeyringError::Unavailable),
            };
            let _ = reply.send(result);
        }
        Job::Delete { path, reply } => {
            let result = match (keyring, runtime) {
                (Some(keyring), Some(runtime)) => delete(runtime, keyring, &path),
                _ => Err(KeyringError::Unavailable),
            };
            let _ = reply.send(result);
        }
    }
}

fn lookup(
    runtime: &tokio::runtime::Runtime,
    keyring: &oo7::Keyring,
    path: &str,
) -> Result<Option<Zeroizing<Vec<u8>>>, KeyringError> {
    let items = runtime
        .block_on(keyring.search_items(&attributes(path)))
        .map_err(failure)?;
    let Some(item) = items.into_iter().next() else {
        return Ok(None);
    };
    let secret = runtime.block_on(item.secret()).map_err(failure)?;
    Ok(Some(Zeroizing::new(secret.as_bytes().to_vec())))
}

fn store(
    runtime: &tokio::runtime::Runtime,
    keyring: &oo7::Keyring,
    path: &str,
    passphrase: &[u8],
) -> Result<(), KeyringError> {
    runtime
        .block_on(keyring.create_item(
            &label(path),
            &attributes(path),
            oo7::Secret::blob(passphrase),
            true,
        ))
        .map_err(failure)
}

fn delete(
    runtime: &tokio::runtime::Runtime,
    keyring: &oo7::Keyring,
    path: &str,
) -> Result<(), KeyringError> {
    runtime
        .block_on(keyring.delete(&attributes(path)))
        .map_err(failure)
}

fn label(path: &str) -> String {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("journal.quaderno");
    format!("Quaderno vault: {name}")
}

fn failure(error: oo7::Error) -> KeyringError {
    KeyringError::Failure(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_identify_the_vault_path() {
        let attributes = attributes("/home/me/journal.quaderno");
        assert_eq!(
            attributes.get("xdg:schema").map(String::as_str),
            Some(SCHEMA)
        );
        assert_eq!(
            attributes.get("vault-path").map(String::as_str),
            Some("/home/me/journal.quaderno")
        );
    }

    #[test]
    fn the_label_names_the_file() {
        assert_eq!(
            label("/home/me/journal.quaderno"),
            "Quaderno vault: journal.quaderno"
        );
    }
}
