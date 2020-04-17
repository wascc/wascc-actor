// Copyright 2015-2019 Capital One Services, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Errors
//!
//! This module contains tghe object store client interface through which actor modules
//! access a bound `wascc:blobstore` capability provider

use crate::Result;
use codec::blobstore::Blob;
use codec::blobstore::Container;
use codec::blobstore::{BlobList, FileChunk, StreamRequest, Transfer};
use codec::blobstore::{
    OP_CREATE_CONTAINER, OP_GET_OBJECT_INFO, OP_LIST_OBJECTS, OP_REMOVE_CONTAINER,
    OP_REMOVE_OBJECT, OP_START_DOWNLOAD, OP_START_UPLOAD, OP_UPLOAD_CHUNK,
};
use codec::{deserialize, serialize};
use wapc_guest::host_call;
use wascc_codec as codec;

const CAPID_BLOBSTORE: &str = "wascc:blobstore";

/// An abstraction around a host runtime capability for a key-value store
pub struct ObjectStoreHostBinding {
    binding: String,
}

impl Default for ObjectStoreHostBinding {
    fn default() -> Self {
        ObjectStoreHostBinding {
            binding: "default".to_string(),
        }
    }
}

/// Creates a named host binding for the `wascc:objectstore` capability
pub fn host(binding: &str) -> ObjectStoreHostBinding {
    ObjectStoreHostBinding {
        binding: binding.to_string(),
    }
}

/// Creates the default host binding for the `wascc:objectstore` capability
pub fn default() -> ObjectStoreHostBinding {
    ObjectStoreHostBinding::default()
}

impl ObjectStoreHostBinding {
    /// Creates a new container within the store
    pub fn create_container(&self, name: &str) -> Result<Container> {
        let cmd = Container {
            id: name.to_string(),
        };
        host_call(
            &self.binding,
            CAPID_BLOBSTORE,
            OP_CREATE_CONTAINER,
            &serialize(cmd)?,
        )
        .map(|v| deserialize::<Container>(v.as_ref()).unwrap())
        .map_err(|e| e.into())
    }

    /// Removes a container from the store. Whether or not this will fail if the container
    /// has items may be specific to a given provider implementation.
    pub fn remove_container(&self, name: &str) -> Result<()> {
        let cmd = Container {
            id: name.to_string(),
        };
        host_call(
            &self.binding,
            CAPID_BLOBSTORE,
            OP_REMOVE_CONTAINER,
            &serialize(cmd)?,
        )
        .map(|_v| ())
        .map_err(|e| e.into())
    }

    /// Removes an object from a container
    pub fn remove_object(&self, name: &str, container: &str) -> crate::Result<()> {
        let cmd = Blob {
            id: name.to_string(),
            container: container.to_string(),
            byte_size: 0,
        };
        host_call(
            &self.binding,
            CAPID_BLOBSTORE,
            OP_REMOVE_OBJECT,
            &serialize(cmd)?,
        )
        .map(|_v| ())
        .map_err(|e| e.into())
    }

    /// Lists all objects within a container
    pub fn list_objects(&self, container: &str) -> Result<BlobList> {
        let cmd = Container {
            id: container.to_string(),
        };
        host_call(
            &self.binding,
            CAPID_BLOBSTORE,
            OP_LIST_OBJECTS,
            &serialize(cmd)?,
        )
        .map(|v| deserialize::<BlobList>(v.as_ref()).unwrap())
        .map_err(|e| e.into())
    }

    /// Obtains binary object metadata, does not include the object bytes
    pub fn get_blob_info(&self, container: &str, id: &str) -> Result<Option<Blob>> {
        let cmd = Blob {
            id: id.to_string(),
            container: container.to_string(),
            byte_size: 0,
        };
        host_call(
            &self.binding,
            CAPID_BLOBSTORE,
            OP_GET_OBJECT_INFO,
            &serialize(cmd)?,
        )
        .map(|v| {
            let b = deserialize::<Blob>(v.as_ref()).unwrap();
            if b.id.is_empty() {
                None
            } else {
                Some(b)
            }
        })
        .map_err(|e| e.into())
    }

    /// Indicates that an upload is about to begin for an item. You should follow this
    /// call up with a for loop/iteration that sends successive chunks to the store. The chunk
    /// size specified in this call is a request or suggestion. It is up to the provider to determine
    /// the actual chunk size, which is returned in the resulting `Transfer` instance
    pub fn start_upload(&self, blob: &Blob, chunk_size: u64, total_bytes: u64) -> Result<Transfer> {
        let transfer = Transfer {
            blob_id: blob.id.to_string(),
            container: blob.container.to_string(),
            chunk_size,
            total_size: total_bytes,
            total_chunks: total_bytes / chunk_size,
        };
        let cmd = FileChunk {
            sequence_no: 0,
            container: blob.container.to_string(),
            id: blob.id.to_string(),
            chunk_size,
            total_bytes,
            chunk_bytes: vec![],
        };
        host_call(
            &self.binding,
            CAPID_BLOBSTORE,
            OP_START_UPLOAD,
            &serialize(cmd)?,
        )
        .map(|_v| transfer)
        .map_err(|e| e.into())
    }

    /// Uploads an individual chunk of a file to the blob store. This call must only ever
    /// come after signaling the start of a new upload with the `start_upload` function.
    pub fn upload_chunk(
        &self,
        transfer: &Transfer,
        offset: u64,
        bytes: &[u8],
    ) -> crate::Result<()> {
        let cmd = FileChunk {
            id: transfer.blob_id.to_string(),
            container: transfer.container.to_string(),
            sequence_no: offset,
            chunk_size: transfer.chunk_size,
            total_bytes: transfer.total_size,
            chunk_bytes: bytes.to_vec(),
        };
        host_call(
            &self.binding,
            CAPID_BLOBSTORE,
            OP_UPLOAD_CHUNK,
            &serialize(cmd)?,
        )
        .map(|_v| ())
        .map_err(|e| e.into())
    }

    /// Sends a request to the provider to begin a chunked download of a file. If this
    /// succeeds, your actor will begin receiving `OP_RECEIVE_CHUNK` messages from the
    /// provider.
    pub fn start_download(&self, blob: &Blob, chunk_size: u64) -> crate::Result<Transfer> {
        let transfer = Transfer {
            blob_id: blob.id.to_string(),
            container: blob.container.to_string(),
            chunk_size,
            total_size: blob.byte_size,
            total_chunks: blob.byte_size / chunk_size,
        };
        let cmd = StreamRequest {
            container: blob.container.to_string(),
            id: blob.id.to_string(),
            chunk_size,
        };
        host_call(
            &self.binding,
            CAPID_BLOBSTORE,
            OP_START_DOWNLOAD,
            &serialize(cmd)?,
        )
        .map(|_v| transfer)
        .map_err(|e| e.into())
    }
}
