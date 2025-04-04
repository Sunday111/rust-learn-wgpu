use async_std::path::PathBuf;
use cfg_if::cfg_if;
use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

#[cfg(target_arch = "wasm32")]
fn format_url<P: AsRef<std::path::Path>>(file_name: P) -> anyhow::Result<reqwest::Url> {
    let window: web_sys::Window = web_sys::window().unwrap();
    let location: String = window.location().href().unwrap();
    let location_path = std::path::Path::new(&location[..location.rfind("/").unwrap() + 1]);
    let path = location_path.join("res").join(file_name);
    match path.to_str() {
        Some(path_str) => Ok(reqwest::Url::parse(path_str)?),
        None => Err(anyhow::anyhow!("Could not convert path {:?} to str", path)),
    }
}

pub async fn load_string<P: AsRef<std::path::Path>>(file_name: P) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name)?;
            Ok(reqwest::get(url)
                .await?
                .text()
                .await?)
        } else {
            let path = std::path::PathBuf::from(env!("OUT_DIR")).join("res").join(file_name);
            std::fs::read_to_string(&path).map_err(|err| anyhow::anyhow!("Failed to read {:?}. Error: {:?}", path, err))
        }
    }
}

pub async fn load_binary<P: AsRef<std::path::Path>>(file_name: P) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name)?;
            Ok(reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec())
        } else {
            let path = std::path::PathBuf::from(env!("OUT_DIR")).join("res").join(file_name);
            std::fs::read(&path).map_err(|err| anyhow::anyhow!("Failed to read {:?}. Error: {:?}", path, err))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EndpointId(u32);

struct PendingFile {
    // it's an array because multiple places might be waiting for the same file
    callbacks: Vec<Box<dyn FnOnce(&FileDataHandle)>>,
}

#[derive(Clone, Debug)]
pub struct FileData {
    pub id: FileId,
    pub data: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
pub type FileDataHandle = Rc<FileData>; // Use `Rc<T>` in WebAssembly

#[cfg(not(target_arch = "wasm32"))]
pub type FileDataHandle = std::sync::Arc<FileData>; // Use `Arc<T>` in native platforms

#[derive(Clone)]
pub struct FileLoader {
    inner: Rc<RefCell<FileLoaderInner>>,
}

pub struct FileLoaderInner {
    sender: async_channel::Sender<(String, Vec<u8>)>,
    receiver: async_channel::Receiver<(String, Vec<u8>)>,

    file_id_map: bimap::BiHashMap<String, FileId>,
    next_file_id: FileId,

    endpoint_id_map: HashMap<EndpointId, async_channel::Sender<FileDataHandle>>,
    next_endpoint_id: EndpointId,

    ready_files: HashMap<FileId, FileDataHandle>,
    pending_files: HashMap<FileId, Rc<RefCell<PendingFile>>>,
}

impl FileLoaderInner {
    fn find_file_id(&self, path: &str) -> Option<FileId> {
        match self.file_id_map.get_by_left(path) {
            Some(id) => Some(*id),
            None => None,
        }
    }

    fn find_or_add_file_id(&mut self, path: &str) -> FileId {
        match self.find_file_id(path) {
            Some(id) => id,
            None => {
                let id = self.next_file_id;
                self.next_file_id = FileId(self.next_file_id.0 + 1);
                self.file_id_map.insert(path.into(), id);
                id
            }
        }
    }
}

impl FileLoader {
    pub fn path_by_id(&self, id: FileId) -> Option<String> {
        match self.inner.borrow().file_id_map.get_by_right(&id) {
            Some(s) => Some(s.clone()),
            None => None,
        }
    }

    pub fn new() -> Self {
        let (sender, receiver) = async_channel::unbounded::<(String, Vec<u8>)>();
        Self {
            inner: Rc::new(RefCell::new(FileLoaderInner {
                sender,
                receiver,
                file_id_map: bimap::BiHashMap::new(),
                next_file_id: FileId(0),
                endpoint_id_map: HashMap::new(),
                next_endpoint_id: EndpointId(0),
                ready_files: HashMap::new(),
                pending_files: HashMap::new(),
            })),
        }
    }

    pub fn try_get_file(&self, path: &str) -> Option<FileDataHandle> {
        let inner = self.inner.borrow_mut();

        match inner.file_id_map.get_by_left(path) {
            Some(id) => match inner.ready_files.get(id) {
                Some(file) => Some(file.clone()),
                None => None,
            },
            None => None,
        }
    }

    pub fn get_or_request<Callback>(&mut self, path: &str, callback: Callback) -> FileId
    where
        Callback: 'static + FnOnce(&FileDataHandle),
    {
        let mut inner = self.inner.borrow_mut();

        let id = inner.find_or_add_file_id(path);

        if let Some(file_data) = inner.ready_files.get(&id) {
            callback(file_data);
            return id;
        }

        if let Some(pending) = inner.pending_files.get(&id) {
            pending.borrow_mut().callbacks.push(Box::new(callback));
            return id;
        }

        inner.pending_files.insert(
            id,
            Rc::new(RefCell::new(PendingFile {
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

        return id;
    }

    pub fn poll(&mut self) {
        let mut inner = self.inner.borrow_mut();
        while let Ok((path, data)) = inner.receiver.try_recv() {
            let id = inner.find_or_add_file_id(&path);
            let removed_entry = inner.pending_files.remove_entry(&id);

            if let None = removed_entry {
                log::error!(
                    "Receiver got data for \"{}\", but did not expect that",
                    &path
                );
            }

            match inner.ready_files.get(&id) {
                Some(_) => {
                    log::error!("Got data for \"{}\" but data was already cached", &path);
                }
                None => {
                    // Add to ready files
                    inner
                        .ready_files
                        .insert(id, FileDataHandle::new(FileData { id, data }));
                }
            }

            if let Some((removed_key, pending)) = removed_entry {
                match inner.ready_files.get(&removed_key) {
                    Some(file_data) => {
                        for callback in std::mem::take(&mut pending.borrow_mut().callbacks) {
                            callback(&file_data);
                        }
                    }
                    _ => {
                        log::error!(
                            "Something went very wrong here: succesfully inserted file data for \"{}\" but failed to find it a few calls later",
                            removed_key.0
                        );
                    }
                }
            }
        }
    }

    pub fn make_endpoint(&mut self) -> FileLoaderEndpoint {
        let (id, receiver) = {
            let (sender, receiver) = async_channel::unbounded::<FileDataHandle>();
            let mut inner = self.inner.borrow_mut();
            let id = inner.next_endpoint_id;
            inner.next_endpoint_id = EndpointId(id.0 + 1);
            inner.endpoint_id_map.insert(id, sender);
            (id, receiver)
        };
        FileLoaderEndpoint {
            id,
            receiver,
            loader: FileLoader {
                inner: self.inner.clone(),
            },
        }
    }
}

pub struct FileLoaderEndpoint {
    pub loader: FileLoader,
    id: EndpointId,
    pub receiver: async_channel::Receiver<FileDataHandle>,
}

impl FileLoaderEndpoint {
    pub fn request(&mut self, path: &str) {
        let sender = self
            .loader
            .inner
            .borrow()
            .endpoint_id_map
            .get(&self.id)
            .expect("Endpoint wasn't registered?")
            .clone();
        self.loader.get_or_request(path, move |x| {
            let x = x.clone();
            let loader_fn = async move {
                match sender.send(x.clone()).await {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("Failed to load . Error: \"{}\"", err);
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
        });
    }
}

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

        let expected: String = "why hello".into();
        assert_eq!(loader.path_by_id(FileId(0)), Some(expected));
    }
}
