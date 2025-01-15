use crate::common::delegate::UploadDelegate;
use crate::common::delegate::UploadDelegateConfig;
use crate::common::hub_helper;
use crate::common::table;
use crate::common::table::Table;
use crate::hub::Hub;
use google_drive3::api::Drive;
use std::cmp::min;
use std::error;
use std::fmt;
use std::io;

const MAX_PAGE_SIZE: usize = 100;

pub struct Config {
    pub max_drives: usize,
    pub skip_header: bool,
    pub field_separator: String,
}

pub async fn list(config: Config) -> Result<(), Error> {
    let hub = hub_helper::get_hub().await.map_err(Error::Hub)?;
    let delegate_config = UploadDelegateConfig::default();

    let drives = list_drives(&hub, &config, delegate_config)
        .await
        .map_err(Error::ListDrives)?;

    print_drives_table(&config, drives);

    Ok(())
}

fn print_drives_table(config: &Config, drives: Vec<google_drive3::api::Drive>) {
    let mut values: Vec<[String; 2]> = vec![];

    for drive in drives {
        values.push([
            // fmt
            drive.id.unwrap_or_default(),
            drive.name.unwrap_or_default(),
        ])
    }

    let table = Table {
        header: ["Id", "Name"],
        values,
    };

    let _ = table::write(
        io::stdout(),
        table,
        &table::DisplayConfig {
            skip_header: config.skip_header,
            separator: config.field_separator.clone(),
        },
    );
}

pub async fn list_drives(
    hub: &Hub,
    config: &Config,
    delegate_config: UploadDelegateConfig,
) -> Result<Vec<google_drive3::api::Drive>, google_drive3::Error> {
    let mut delegate = UploadDelegate::new(delegate_config);
    let mut collected_drives: Vec<Drive> = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let max_drives = config.max_drives - collected_drives.len();
        let page_size = min(MAX_PAGE_SIZE, max_drives);

        let mut request = hub
            .drives()
            .list()
            .add_scope(google_drive3::api::Scope::Full)
            .delegate(&mut delegate);

        // If there's a next page, set the page token
        if let Some(token) = page_token {
            request = request.page_token(&token);
        }

        let (_, drives_list) = request.page_size(page_size as i32).doit().await?;

        // Collect drives from the current page
        if let Some(mut drives) = drives_list.drives {
            collected_drives.append(&mut drives);
        }

        // Check if there's another page
        page_token = drives_list.next_page_token;

        if collected_drives.len() >= config.max_drives || page_token.is_none() {
            break;
        }
    }

    let max_drives = min(config.max_drives, collected_drives.len());
    Ok(collected_drives[0..max_drives].to_vec())
}

#[derive(Debug)]
pub enum Error {
    Hub(hub_helper::Error),
    ListDrives(google_drive3::Error),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Hub(err) => write!(f, "{}", err),
            Error::ListDrives(err) => {
                write!(f, "Failed to list drives: {}", err)
            }
        }
    }
}
