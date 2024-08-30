use tf_asset_loader::{Loader, LoaderError};

fn main() -> Result<(), LoaderError> {
    let loader = Loader::new()?;
    if let Some(model) = loader.load("models/props_gameplay/resupply_locker.mdl")? {
        println!("resupply_locker.mdl is {} bytes large", model.len());
    }
    Ok(())
}
