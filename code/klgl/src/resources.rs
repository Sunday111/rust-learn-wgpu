use cfg_if::cfg_if;
use derive_more::Display;
use std::{cell::RefCell, rc::Rc};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
pub struct FileId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
pub struct EndpointId(u32);

struct PendingFile {
    id: FileId,
    // it's an array because multiple places might be waiting for the same file
    callbacks: Vec<Box<dyn FnOnce(&Rc<FileData>)>>,
}

#[derive(Clone, Debug)]
pub struct FileData {
    pub id: FileId,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub struct FileLoader {
    inner: Rc<RefCell<FileLoaderInner>>,
}

pub struct FileLoaderInner {
    sender: async_channel::Sender<(String, Vec<u8>)>,
    receiver: async_channel::Receiver<(String, Vec<u8>)>,

    name_id_map: bimap::BiHashMap<String, FileId>,
    ready_resources: std::collections::HashMap<FileId, Rc<FileData>>,
    pending_resources: std::collections::HashMap<FileId, Rc<RefCell<PendingFile>>>,

    next_id: FileId,
}

impl FileLoaderInner {
    fn find_id(&self, path: &str) -> Option<FileId> {
        match self.name_id_map.get_by_left(path) {
            Some(id) => Some(*id),
            None => None,
        }
    }

    fn find_or_add_id(&mut self, path: &str) -> FileId {
        match self.find_id(path) {
            Some(id) => id,
            None => {
                let id = self.next_id;
                self.next_id = FileId(self.next_id.0 + 1);
                self.name_id_map.insert(path.into(), id);
                id
            }
        }
    }
}

impl FileLoader {
    pub fn new() -> Self {
        let (sender, receiver) = async_channel::unbounded::<(String, Vec<u8>)>();
        Self {
            inner: Rc::new(RefCell::new(FileLoaderInner {
                sender,
                receiver,
                name_id_map: bimap::BiHashMap::new(),
                ready_resources: std::collections::HashMap::new(),
                pending_resources: std::collections::HashMap::new(),
                next_id: FileId(0),
            })),
        }
    }

    pub fn try_get_resource(&self, path: &str) -> Option<Rc<FileData>> {
        let inner = self.inner.borrow_mut();

        match inner.name_id_map.get_by_left(path) {
            Some(id) => match inner.ready_resources.get(id) {
                Some(resource) => Some(resource.clone()),
                None => None,
            },
            None => None,
        }
    }

    pub fn get_or_request<Callback>(&mut self, path: &str, callback: Callback)
    where
        Callback: 'static + FnOnce(&Rc<FileData>),
    {
        let mut inner = self.inner.borrow_mut();

        let id = inner.find_or_add_id(path);

        if let Some(file_data) = inner.ready_resources.get(&id) {
            return callback(file_data);
        }

        if let Some(pending) = inner.pending_resources.get(&id) {
            pending.borrow_mut().callbacks.push(Box::new(callback));
            return;
        }

        inner.pending_resources.insert(
            id,
            Rc::new(RefCell::new(PendingFile {
                id,
                callbacks: vec![Box::new(callback)],
            })),
        );

        let sender_clone = inner.sender.clone();
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

    pub fn poll(&mut self) {
        let mut inner = self.inner.borrow_mut();
        while let Ok((path, data)) = inner.receiver.try_recv() {
            let id = inner.find_or_add_id(&path);
            let removed_entry = inner.pending_resources.remove_entry(&id);

            if let None = removed_entry {
                log::error!(
                    "Receiver got data for \"{}\", but did not expect that",
                    &path
                );
            }

            match inner.ready_resources.get(&id) {
                Some(_) => {
                    log::error!("Got data for \"{}\" but data was already cached", &path);
                }
                None => {
                    // Add to ready resources
                    inner
                        .ready_resources
                        .insert(id, Rc::new(FileData { id, data }));
                }
            }

            if let Some((removed_key, pending)) = removed_entry {
                match inner.ready_resources.get(&removed_key) {
                    Some(file_data) => {
                        for callback in std::mem::take(&mut pending.borrow_mut().callbacks) {
                            callback(&file_data);
                        }
                    }
                    _ => {
                        log::error!(
                            "Something went very wrong here: succesfully inserted file data for \"{}\" but failed to find it a few calls later",
                            removed_key
                        );
                    }
                }
            }
        }
    }
}

// struct FileLoaderEndpoint {
//     loader: FileLoader,
//     receiver: async_channel::Receiver<Rc<FileData>>,
// }

// impl FileLoaderEndpoint {

// }

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_add() {
        let mut loader = FileLoader::new();
        loader.get_or_request("why hello", |x| {
            println!("ready: {:?}", x);
        });
        loader.poll();
    }
}
