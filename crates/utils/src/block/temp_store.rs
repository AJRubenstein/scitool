use std::path::Path;

use bytes::Buf;

use crate::buffer::Buffer;

use super::BlockSource;

pub struct TempStore {
    temp_dir: tempfile::TempDir,
}

impl TempStore {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            temp_dir: tempfile::TempDir::new()?,
        })
    }

    pub fn with_base(base: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            temp_dir: tempfile::TempDir::new_in(base)?,
        })
    }

    pub async fn store_bytes<B>(&mut self, buffer: B) -> anyhow::Result<BlockSource>
    where
        B: Buf,
    {
        self.create_temp_block(buffer).await
    }

    pub async fn store<B>(&mut self, buffer: B) -> anyhow::Result<BlockSource>
    where
        B: Buffer + Send + Sync + 'static,
    {
        self.create_temp_block(buffer.lock()?).await
    }

    async fn create_temp_block<B>(&self, buffer: B) -> anyhow::Result<BlockSource>
    where
        B: Buf,
    {
        let (mut file, path) = tempfile::NamedTempFile::new_in(self.temp_dir.path())?.keep()?;
        std::io::copy(&mut buffer.reader(), &mut file)?;
        drop(file);
        Ok(BlockSource::from_path(path)?)
    }
}

#[cfg(test)]
mod tests {
    use bytes::BufMut;

    use super::*;
    use crate::block::MemBlock;
    use macro_rules_attribute::apply;
    use smol_macros::test;

    #[apply(test!)]
    async fn test_temp_store() -> anyhow::Result<()> {
        let mut store = TempStore::new()?;
        let buffer = MemBlock::from_vec(vec![1, 2, 3, 4]);
        let block_source = store.store(buffer).await?;
        assert_eq!(block_source.size(), 4);
        let mut read_data = Vec::new();
        read_data.put(block_source.lock().unwrap());
        assert_eq!(read_data, vec![1, 2, 3, 4]);
        Ok(())
    }
}
