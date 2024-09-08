#![expect(dead_code)]

use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use res::{
    datafile::{Contents, DataFile, RawContents},
    mapfile::ResourceLocations,
    ResourceId, ResourceType,
};
use util::{
    block::{Block, BlockReader, BlockSource},
    data_writer::{DataWriter, IoDataWriter},
};

mod res;
mod util;

struct ResourceStore {
    resource_locations: ResourceLocations,
    data_file: DataFile,
}

impl ResourceStore {
    fn open(map_file: &Path, data_file: &Path) -> io::Result<Self> {
        let map_file = Block::from_reader(File::open(map_file)?)?;
        let data_file = DataFile::new(BlockSource::from_path(data_file)?);
        let resource_locations =
            res::mapfile::ResourceLocations::read_from(BlockReader::new(map_file))?;
        Ok(ResourceStore {
            resource_locations,
            data_file,
        })
    }

    pub fn read_raw_contents(&self) -> impl Iterator<Item = io::Result<RawContents>> + '_ {
        self.resource_locations.locations().map(move |location| {
            let raw_contents = self.data_file.read_raw_contents(&location)?;
            Ok(raw_contents)
        })
    }

    pub fn read_resource(&self, res_type: ResourceType, res_num: u16) -> io::Result<Contents> {
        let location = self
            .resource_locations
            .get_location(&ResourceId::new(res_type, res_num))
            .unwrap();
        Ok(self.data_file.read_contents(&location)?)
    }
}

fn open_main_store(root_dir: &Path) -> anyhow::Result<ResourceStore> {
    let map_file = root_dir.join("RESOURCE.MAP");
    let data_file = root_dir.join("RESOURCE.000");
    Ok(ResourceStore::open(&map_file, &data_file)?)
}

fn open_message_store(root_dir: &Path) -> anyhow::Result<ResourceStore> {
    let map_file = root_dir.join("MESSAGE.MAP");
    let data_file = root_dir.join("RESOURCE.MSG");
    Ok(ResourceStore::open(&map_file, &data_file)?)
}

#[derive(Parser)]
struct ListResources {
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl ListResources {
    fn run(&self) -> anyhow::Result<()> {
        let resource_dir_files = open_main_store(&self.root_dir.join("R"))?;
        for item in resource_dir_files.read_raw_contents() {
            let header = item?;
            println!("{:?}", header);
        }
        Ok(())
    }
}

#[derive(Parser)]
struct ListMessageResources {
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl ListMessageResources {
    fn run(&self) -> anyhow::Result<()> {
        let resource_dir_files = open_message_store(&self.root_dir)?;
        for item in resource_dir_files.read_raw_contents() {
            let header = item?;
            println!("{:?}", header);
        }
        Ok(())
    }
}

#[derive(Parser)]
struct ExtractResourceAsPatch {
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[clap(index = 2)]
    resource_type: ResourceType,
    #[clap(index = 3)]
    resource_id: u16,
    #[clap(short = 'n', long, default_value = "false")]
    dry_run: bool,
    #[clap(short = 'o', long)]
    output_dir: Option<PathBuf>,
}

impl ExtractResourceAsPatch {
    fn run(&self) -> anyhow::Result<()> {
        let resource_dir_files = open_main_store(&self.root_dir)?;
        let contents = resource_dir_files.read_resource(self.resource_type, self.resource_id)?;
        let ext = match self.resource_type {
            ResourceType::Script => "SCR",
            ResourceType::Heap => "HEP",
            _ => {
                anyhow::bail!("Unsupported resource type");
            }
        };

        let out_root = self.output_dir.as_ref().unwrap_or(&self.root_dir);

        let filename = out_root.join(format!("{0}.{1}", self.resource_id, ext));
        if self.dry_run {
            eprintln!(
                "DRY_RUN: Writing resource {restype:?}:{resid} to {filename:?}",
                restype = self.resource_type,
                resid = self.resource_id,
                filename = filename
            );
        } else {
            eprintln!(
                "Writing resource {restype:?}:{resid} to {filename:?}",
                restype = self.resource_type,
                resid = self.resource_id,
                filename = filename
            );
            {
                let mut patch_file = IoDataWriter::new(
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(self.root_dir.join(filename))?,
                );

                patch_file.write_u8(self.resource_type.into())?;
                patch_file.write_u8(0)?; // Header Size
                patch_file.write_block(&contents.data().open()?)?;
            }
        }

        Ok(())
    }
}

#[derive(Subcommand)]
enum ResourceCommand {
    #[clap(name = "list")]
    List(ListResources),
    ListMsg(ListMessageResources),
    ExtractAsPatch(ExtractResourceAsPatch),
}

impl ResourceCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ResourceCommand::List(list) => list.run()?,
            ResourceCommand::ExtractAsPatch(extract) => extract.run()?,
            ResourceCommand::ListMsg(list_msg) => list_msg.run()?,
        }
        Ok(())
    }
}

#[derive(Parser)]
struct Resource {
    #[clap(subcommand)]
    res_cmd: ResourceCommand,
}

impl Resource {
    fn run(&self) -> anyhow::Result<()> {
        self.res_cmd.run()
    }
}

#[derive(Subcommand)]
enum Category {
    #[clap(name = "res")]
    Resource(Resource),
}

impl Category {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            Category::Resource(res) => res.run(),
        }
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    category: Category,
}

impl Cli {
    fn run(&self) -> anyhow::Result<()> {
        self.category.run()
    }
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    args.run()?;
    Ok(())
}
