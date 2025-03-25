use std::collections::HashMap;

use cfg_if::cfg_if;

#[cfg(target_arch = "wasm32")]
fn format_url(file_name: &str) -> reqwest::Url {
    let window: web_sys::Window = web_sys::window().unwrap();
    let location: String = window.location().href().unwrap();
    let path: &str = &location[..location.rfind("/").unwrap() + 1];
    reqwest::Url::parse(&format!("{}/res/{}", path, file_name)).unwrap()
}

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let txt = reqwest::get(url)
                .await?
                .text()
                .await?;
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("res")
                .join(file_name);

            let txt = std::fs::read_to_string(path)?;
        }
    }

    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let data = reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec();
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("res")
                .join(file_name);
            // log::info!("Reading \"{:?}\"", path);
            let data = std::fs::read(path)?;
        }
    }

    Ok(data)
}

struct PendingResource {
    // it's an array because multiple places might be waiting for the same resouce
    callbacks: Vec<Box<dyn FnOnce(&[u8])>>,
}

struct CachedResource {
    data: Vec<u8>,
}

enum ResourceState {
    Pending(PendingResource),
    Cached(CachedResource),
}

pub struct ResourceLoader {
    sender: async_channel::Sender<(String, Vec<u8>)>,
    receiver: async_channel::Receiver<(String, Vec<u8>)>,
    resources: std::collections::HashMap<String, ResourceState>,
}

impl ResourceLoader {
    pub fn new() -> Self {
        let (sender, receiver) = async_channel::unbounded::<(String, Vec<u8>)>();
        Self {
            sender,
            receiver,
            resources: HashMap::new(),
        }
    }

    pub fn try_get_resource(&self, path: &str) -> Option<&[u8]> {
        match self.resources.get(path) {
            Some(state) => match state {
                ResourceState::Cached(cached) => Some(&cached.data),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn get_or_request_resource<Callback>(&mut self, path: &str, callback: Callback)
    where
        Callback: 'static + FnOnce(&[u8]),
    {
        match self.resources.get_mut(path) {
            Some(state) => match state {
                ResourceState::Cached(cached) => {
                    callback(&cached.data);
                }
                ResourceState::Pending(pending) => {
                    pending.callbacks.push(Box::new(callback));
                }
            },
            None => {
                self.resources.insert(
                    path.into(),
                    ResourceState::Pending(PendingResource {
                        callbacks: vec![Box::new(callback)],
                    }),
                );

                let sender_clone = self.sender.clone();
                let path_clone: String = path.into();
                let loader_fn = async move {
                    match load_binary(&path_clone).await {
                        Ok(data) => {
                            log::info!("Received: \"{}\"", path_clone);
                            let _ = sender_clone.send((path_clone, data)).await;
                        }
                        Err(err) => {
                            log::error!("Failed to load \"{}\". Reason: \"{}\"", path_clone, err);
                        }
                    };
                };

                cfg_if::cfg_if! {
                    if #[cfg(target_arch = "wasm32")] {
                        wasm_bindgen_futures::spawn_local(loader_fn);
                    } else {
                        async_std::task::spawn(loader_fn);
                    }
                }
            }
        };
    }

    pub fn poll(&mut self) {
        while let Ok((path, data)) = self.receiver.try_recv() {
            match self.resources.remove_entry(&path) {
                Some((removed_key, removed_value)) => {
                    self.resources
                        .insert(path, ResourceState::Cached(CachedResource { data: data }));
                    match removed_value {
                        ResourceState::Pending(pending) => {
                            match self.resources.get(&removed_key).unwrap() {
                                ResourceState::Cached(cached) => {
                                    for callback in pending.callbacks {
                                        callback(&cached.data);
                                    }
                                }
                                _ => {
                                    log::error!("Something went very wrong here.");
                                }
                            }
                        }
                        ResourceState::Cached(_) => {
                            log::error!(
                                "Receiver got data for \"{}\", but this resource is cached",
                                removed_key
                            );
                        }
                    }
                }
                None => {
                    log::error!(
                        "Receiver got data for \"{}\", but did not expect that",
                        path
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_add() {
        let mut loader = ResourceLoader::new();
        loader.get_or_request_resource("why hello", |x| {
            println!("ready: {:?}", x);
        });
    }
}
