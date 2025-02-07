use crate::common::delegate::UploadDelegate;
use crate::common::delegate::UploadDelegateConfig;
use crate::common::hub_helper;
use crate::common::permission;
use crate::files;
use crate::hub::Hub;
use std::error;
use std::fmt::Display;
use std::fmt::Formatter;

#[derive(Clone, Debug)]
pub struct Config {
    pub role: permission::Role,
    pub type_: permission::Type,
    pub discoverable: bool,
    pub domain: Option<String>,
    pub notify_on_share: bool,
}

impl Config {
    fn allow_file_discovery(&self) -> Option<bool> {
        if self.type_.supports_file_discovery() {
            Some(self.discoverable)
        } else {
            None
        }
    }

    fn requires_ownership_transfer(&self) -> bool {
        self.role == permission::Role::Owner
    }
}

pub async fn share(
    config: Config,
    file_ids: Vec<String>,
    emails: Option<Vec<String>>,
) -> Result<(), Error> {
    err_if_missing_email(&config, emails.clone())?;
    err_if_missing_domain(&config)?;

    let hub = hub_helper::get_hub().await.map_err(Error::Hub)?;
    let delegate_config = UploadDelegateConfig::default();
    let emails_vec = emails.unwrap_or(vec!["".to_string()]);

    for file_id in file_ids {
        let file = files::info::get_file(&hub, &file_id)
            .await
            .map_err(Error::GetFile)?;

        for email in &emails_vec {
            create_permission(
                &hub,
                delegate_config,
                &file_id,
                config.role,
                config.type_,
                config.allow_file_discovery(),
                config.requires_ownership_transfer(),
                match email.as_str() {
                    "" => None,
                    _ => Some(email.clone()),
                },
                config.domain.clone(),
            )
            .await
            .map_err(Error::CreatePermission)?;

            print_grant_details(&file, &config, Some(email.clone()));
        }
    }

    Ok(())
}

pub async fn create_permission(
    hub: &Hub,
    delegate_config: UploadDelegateConfig,
    file_id: &str,
    role: permission::Role,
    perm_type: permission::Type,
    allow_file_discovery: Option<bool>,
    requires_ownership_transfer: bool,
    email_address: Option<String>,
    domain: Option<String>,
) -> Result<google_drive3::api::Permission, google_drive3::Error> {
    let mut delegate = UploadDelegate::new(delegate_config);

    let new_permission = google_drive3::api::Permission {
        role: Some(role.to_string()),
        type_: Some(perm_type.to_string()),
        allow_file_discovery,
        email_address,
        domain,
        ..google_drive3::api::Permission::default()
    };

    let (_, permission) = hub
        .permissions()
        .create(new_permission.clone(), &file_id)
        .param(
            "fields",
            "id,role,type,domain,emailAddress,allowFileDiscovery",
        )
        .param(
            "sendNotificationEmail",
            "false",
        )
        .transfer_ownership(requires_ownership_transfer)
        .add_scope(google_drive3::api::Scope::Full)
        .delegate(&mut delegate)
        .supports_all_drives(true)
        .doit()
        .await?;

    Ok(permission)
}

#[derive(Debug)]
pub enum Error {
    Hub(hub_helper::Error),
    GetFile(google_drive3::Error),
    CreatePermission(google_drive3::Error),
    MissingEmail(permission::Type),
    MissingDomain(permission::Type),
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Error::Hub(err) => write!(f, "{}", err),
            Error::GetFile(err) => {
                write!(f, "Failed to get file: {}", err)
            }
            Error::CreatePermission(err) => {
                write!(f, "Failed to share file: {}", err)
            }
            Error::MissingEmail(type_) => {
                write!(
                    f,
                    "Email is required for permission type '{}'. Use the --email option",
                    type_
                )
            }
            Error::MissingDomain(type_) => {
                write!(
                    f,
                    "Domain is required for permission type '{}'. Use the --domain option",
                    type_
                )
            }
        }
    }
}

fn err_if_missing_email(config: &Config, emails: Option<Vec<String>>) -> Result<(), Error> {
    if config.type_.requires_email() && (emails.is_none() || emails.is_some_and(|v| v.is_empty())) {
        return Err(Error::MissingEmail(config.type_.clone()));
    }

    Ok(())
}

fn err_if_missing_domain(config: &Config) -> Result<(), Error> {
    if config.type_.requires_domain() && config.domain.is_none() {
        return Err(Error::MissingDomain(config.type_.clone()));
    }

    Ok(())
}

fn print_grant_details(file: &google_drive3::api::File, config: &Config, email: Option<String>) {
    if config.type_.requires_domain() {
        println!(
            "Granting '{}' permission to {} '{}' for '{}'",
            config.role,
            config.type_,
            config.domain.clone().unwrap_or_default(),
            file.name.clone().unwrap_or_default()
        );
    } else if config.type_.requires_email() {
        println!(
            "Granting '{}' permission to '{}' with email '{}' for '{}'",
            config.role,
            config.type_,
            email.unwrap_or_default(),
            file.name.clone().unwrap_or_default()
        );
    } else {
        println!(
            "Granting '{}' permission to '{}' for '{}'",
            config.role,
            config.type_,
            file.name.clone().unwrap_or_default()
        );
    }
}
