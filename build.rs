use std::io;
#[cfg(windows)] use winres::WindowsResource;

fn main() -> io::Result<()> {
    #[cfg(windows)] {
        WindowsResource::new()
            .set("FileDescription", "Minecraft Scraper, for scraping player count and ping and exporting it to InfluxDB.")
            .set("ProductName", "Minecraft Scraper")
            .set("OriginalFilename", "mcscrape.exe")
            .set("LegalCopyright", "Copyright (c) 2022, Valaphee.")
            .set("CompanyName", "Valaphee")
            .set("InternalName", "mcscrape.exe")
            .set_icon("mcscrape.ico")
            /*.set_resource_file("mcscrape.rc")*/
            .compile()?;
    }
    Ok(())
}
