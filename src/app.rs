// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::Limits;
use cosmic::iced_winit::commands::popup::{destroy_popup, get_popup};
use cosmic::widget::{self, settings};
use cosmic::{Application, Element};

use crate::fl;
use crate::core::openvpn::{get_openvpn_profiles, is_openvpn3_available, delete_openvpn_profile, disconnect_openvpn_session, import_openvpn_config, start_openvpn_session, is_profile_connecting, OpenVPNProfile};

/// This is the struct that represents your application.
/// It is used to define the data that will be used by your application.
#[derive(Default)]
pub struct OpenVpn3Status {
    /// Application state which is managed by the COSMIC runtime.
    core: Core,
    /// The popup id.
    popup: Option<Id>,
    /// Example row toggler.
    example_row: bool,
    /// OpenVPN 3 profiles available on the system.
    openvpn_profiles: Vec<OpenVPNProfile>,
    /// Whether OpenVPN 3 is available on the system.
    openvpn3_available: bool,
    /// Import dialog popup id.
    import_dialog: Option<Id>,
    /// File path for import dialog.
    import_file_path: String,
    /// Custom name for import dialog.
    import_custom_name: String,
    /// Connect dialog popup id.
    connect_dialog: Option<Id>,
    /// Profile name for connect dialog.
    connect_profile_name: String,
    /// Username for connect dialog.
    connect_username: String,
    /// Password for connect dialog.
    connect_password: String,
    /// TOTP for connect dialog.
    connect_totp: String,
}

/// This is the enum that contains all the possible variants that your application will need to transmit messages.
/// This is used to communicate between the different parts of your application.
/// If your application does not need to send messages, you can use an empty enum or `()`.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    // ToggleExampleRow(bool), // Example row - commented out but kept for reference
    DeleteProfile(String), // Profile name to delete
    DisconnectSession(String), // Profile name to disconnect
    RefreshProfiles, // Refresh OpenVPN profiles and session status
    ImportConfig, // Open import dialog
    ImportDialogClosed(Id), // Import dialog was closed
    ImportFilePathChanged(String), // File path input changed
    ImportCustomNameChanged(String), // Custom name input changed
    ImportConfigSubmit, // Submit the import form
    ConfigImported(String, Option<String>), // Config file path and custom name that was imported
    ConnectProfile(String), // Open connect dialog for profile
    ConnectDialogClosed(Id), // Connect dialog was closed
    ConnectUsernameChanged(String), // Username input changed
    ConnectPasswordChanged(String), // Password input changed
    ConnectTotpChanged(String), // TOTP input changed
    ConnectSubmit, // Submit the connect form
    SessionStarted(String), // Session started for profile
    AutoRefresh, // Automatic refresh triggered by connecting states
}

/// Implement the `Application` trait for your application.
/// This is where you define the behavior of your application.
///
/// The `Application` trait requires you to define the following types and constants:
/// - `Executor` is the async executor that will be used to run your application's commands.
/// - `Flags` is the data that your application needs to use before it starts.
/// - `Message` is the enum that contains all the possible variants that your application will need to transmit messages.
/// - `APP_ID` is the unique identifier of your application.
impl Application for OpenVpn3Status {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = "com.system76.OpenVPN3Status";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// This is the entry point of your application, it is where you initialize your application.
    ///
    /// Any work that needs to be done before the application starts should be done here.
    ///
    /// - `core` is used to passed on for you by libcosmic to use in the core of your own application.
    /// - `flags` is used to pass in any data that your application needs to use before it starts.
    /// - `Command` type is used to send messages to your application. `Command::none()` can be used to send no messages to your application.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let openvpn3_available = is_openvpn3_available();
        
        // Load OpenVPN profiles synchronously if available
        let openvpn_profiles = if openvpn3_available {
            get_openvpn_profiles().unwrap_or_default()
        } else {
            Vec::new()
        };
        
        let app = OpenVpn3Status {
            core,
            openvpn_profiles,
            openvpn3_available,
            ..Default::default()
        };

        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// This is the main view of your application, it is the root of your widget tree.
    ///
    /// The `Element` type is used to represent the visual elements of your application,
    /// it has a `Message` associated with it, which dictates what type of message it can send.
    ///
    /// To get a better sense of which widgets are available, check out the `widget` module.
    fn view(&self) -> Element<'_, Self::Message> {
        self.core
            .applet
            .icon_button("display-symbolic")
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let mut content_list = widget::list_column()
            .padding(5)
            .spacing(0);
            // .add(settings::item(
            //     fl!("example-row"),
            //     widget::toggler(self.example_row).on_toggle(Message::ToggleExampleRow),
            // ));

        // Add OpenVPN 3 profiles if available
        if self.openvpn3_available {
            // Always add header for profiles section with refresh and import buttons
            let refresh_button = widget::button::icon(widget::icon::from_name("view-refresh-symbolic"))
                .on_press(Message::RefreshProfiles);
            let import_button = widget::button::icon(widget::icon::from_name("document-open-symbolic"))
                .on_press(Message::ImportConfig);
            
            // Create a row with both buttons
            let button_row = widget::row()
                .push(refresh_button)
                .push(import_button)
                .spacing(5);
            
            content_list = content_list.add(settings::item(
                "OpenVPN 3 Profiles",
                button_row,
            ));
            
            if self.openvpn_profiles.is_empty() {
                content_list = content_list.add(settings::item(
                    "Profiles",
                    widget::text("No profiles found"),
                ));
            } else {
                // Add each profile
                for profile in &self.openvpn_profiles {
                    if profile.is_active {
                        // Show disconnect button for active sessions (use different icon)
                        let disconnect_button = widget::button::icon(widget::icon::from_name("media-playback-stop-symbolic"))
                            .on_press(Message::DisconnectSession(profile.name.clone()));
                        
                        content_list = content_list.add(settings::item(
                            &profile.name,
                            disconnect_button,
                        ));
                    } else {
                        // Show connect and delete buttons for inactive profiles
                        let connect_button = widget::button::icon(widget::icon::from_name("media-playback-start-symbolic"))
                            .on_press(Message::ConnectProfile(profile.name.clone()));
                        let delete_button = widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                            .on_press(Message::DeleteProfile(profile.name.clone()));
                        
                        let button_row = widget::row()
                            .push(connect_button)
                            .push(delete_button)
                            .spacing(5);
                        
                        content_list = content_list.add(settings::item(
                            &profile.name,
                            button_row,
                        ));
                    }
                }
            }
        } else {
            content_list = content_list.add(settings::item(
                "OpenVPN 3",
                widget::text("OpenVPN 3 not available"),
            ));
        }

        // Add import dialog content to the main list if open
        let mut final_content_list = content_list;
        if let Some(import_dialog_id) = self.import_dialog {
            let import_dialog = self.view_import_dialog(import_dialog_id);
            final_content_list = final_content_list.add(settings::item(
                "Import Dialog",
                import_dialog,
            ));
        }
        
        // Add connect dialog content to the main list if open
        if let Some(connect_dialog_id) = self.connect_dialog {
            let connect_dialog = self.view_connect_dialog(connect_dialog_id);
            final_content_list = final_content_list.add(settings::item(
                "Connect Dialog",
                connect_dialog,
            ));
        }
        
        self.core.applet.popup_container(final_content_list).into()
    }

    /// Application messages are handled here. The application state can be modified based on
    /// what message was received. Commands may be returned for asynchronous execution on a
    /// background thread managed by the application's executor.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    // Refresh OpenVPN profiles and session status when opening popup
                    if self.openvpn3_available {
                        self.openvpn_profiles = get_openvpn_profiles().unwrap_or_default();
                    }
                    
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(372.0)
                        .min_width(300.0)
                        .min_height(200.0)
                        .max_height(1080.0);
                    get_popup(popup_settings)
                }
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            // Message::ToggleExampleRow(toggled) => self.example_row = toggled,
            Message::DeleteProfile(profile_name) => {
                // Actually delete the profile from OpenVPN 3 system
                match delete_openvpn_profile(&profile_name) {
                    Ok(_) => {
                        // Remove the profile from the list only if deletion was successful
                        self.openvpn_profiles.retain(|profile| profile.name != profile_name);
                    }
                    Err(error) => {
                        // TODO: Show error message to user
                        eprintln!("Failed to delete profile '{}': {}", profile_name, error);
                    }
                }
            }
            Message::DisconnectSession(profile_name) => {
                // Disconnect the active session
                match disconnect_openvpn_session(&profile_name) {
                    Ok(_) => {
                        // Session disconnected successfully
                        eprintln!("Successfully disconnected session for profile '{}'", profile_name);
                        // Refresh the profiles list to show the updated session status
                        if self.openvpn3_available {
                            self.openvpn_profiles = get_openvpn_profiles().unwrap_or_default();
                        }
                    }
                    Err(error) => {
                        // TODO: Show error message to user
                        eprintln!("Failed to disconnect session for profile '{}': {}", profile_name, error);
                    }
                }
            }
            Message::RefreshProfiles => {
                // Refresh OpenVPN profiles and session status
                if self.openvpn3_available {
                    self.openvpn_profiles = get_openvpn_profiles().unwrap_or_default();
                    eprintln!("Refreshed OpenVPN profiles: {} profiles found", self.openvpn_profiles.len());
                    
                    // Check if any profiles are in connecting state and schedule auto-refresh
                    let has_connecting = self.openvpn_profiles.iter().any(|profile| is_profile_connecting(&profile.name));
                    if has_connecting {
                        eprintln!("Found connecting profiles, scheduling auto-refresh in 3 seconds");
                        return Task::perform(
                            async {
                                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            },
                            |_| cosmic::Action::App(Message::AutoRefresh),
                        );
                    }
                }
            }
            Message::AutoRefresh => {
                // Automatic refresh triggered by connecting states
                if self.openvpn3_available {
                    self.openvpn_profiles = get_openvpn_profiles().unwrap_or_default();
                    eprintln!("Auto-refreshed OpenVPN profiles: {} profiles found", self.openvpn_profiles.len());
                    
                    // Check again if any profiles are still connecting and schedule another refresh
                    let has_connecting = self.openvpn_profiles.iter().any(|profile| is_profile_connecting(&profile.name));
                    if has_connecting {
                        eprintln!("Still found connecting profiles, scheduling another auto-refresh in 3 seconds");
                        return Task::perform(
                            async {
                                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                            },
                            |_| cosmic::Action::App(Message::AutoRefresh),
                        );
                    } else {
                        eprintln!("No more connecting profiles, stopping auto-refresh");
                    }
                }
            }
            Message::ImportConfig => {
                // Open import dialog
                let import_dialog_id = Id::unique();
                self.import_dialog = Some(import_dialog_id);
                self.import_file_path.clear();
                self.import_custom_name.clear();
                eprintln!("Opening import dialog");
            }
            Message::ImportDialogClosed(id) => {
                if self.import_dialog == Some(id) {
                    self.import_dialog = None;
                }
            }
            Message::ImportFilePathChanged(path) => {
                self.import_file_path = path;
            }
            Message::ImportCustomNameChanged(name) => {
                self.import_custom_name = name;
            }
            Message::ImportConfigSubmit => {
                if !self.import_file_path.trim().is_empty() {
                    let custom_name = if self.import_custom_name.trim().is_empty() {
                        None
                    } else {
                        Some(self.import_custom_name.trim())
                    };
                    // Close the dialog
                    self.import_dialog = None;
                    // Import the config
                    match import_openvpn_config(&self.import_file_path, custom_name) {
                        Ok(profile_name) => {
                            eprintln!("Successfully imported config '{}' as profile '{}'", self.import_file_path, profile_name);
                        }
                        Err(error) => {
                            eprintln!("Failed to import config '{}': {}", self.import_file_path, error);
                        }
                    }
                    // Always refresh the profiles list after import attempt
                    if self.openvpn3_available {
                        self.openvpn_profiles = get_openvpn_profiles().unwrap_or_default();
                    }
                }
            }
            Message::ConfigImported(config_path, custom_name) => {
                // Handle imported config file with custom name
                match import_openvpn_config(&config_path, custom_name.as_deref()) {
                    Ok(profile_name) => {
                        eprintln!("Successfully imported config '{}' as profile '{}'", config_path, profile_name);
                        // Refresh the profiles list to show the new profile
                        if self.openvpn3_available {
                            self.openvpn_profiles = get_openvpn_profiles().unwrap_or_default();
                        }
                    }
                    Err(error) => {
                        eprintln!("Failed to import config '{}': {}", config_path, error);
                    }
                }
            }
            Message::ConnectProfile(profile_name) => {
                // Open connect dialog
                let connect_dialog_id = Id::unique();
                self.connect_dialog = Some(connect_dialog_id);
                self.connect_profile_name = profile_name;
                self.connect_username.clear();
                self.connect_password.clear();
                self.connect_totp.clear();
                eprintln!("Opening connect dialog for profile: {}", self.connect_profile_name);
            }
            Message::ConnectDialogClosed(id) => {
                if self.connect_dialog == Some(id) {
                    self.connect_dialog = None;
                }
            }
            Message::ConnectUsernameChanged(username) => {
                self.connect_username = username;
            }
            Message::ConnectPasswordChanged(password) => {
                self.connect_password = password;
            }
            Message::ConnectTotpChanged(totp) => {
                self.connect_totp = totp;
            }
            Message::ConnectSubmit => {
                if !self.connect_username.trim().is_empty() && !self.connect_password.trim().is_empty() {
                    let totp = if self.connect_totp.trim().is_empty() {
                        None
                    } else {
                        Some(self.connect_totp.trim())
                    };
                    // Close the dialog
                    self.connect_dialog = None;
                    // Start the session
                    match start_openvpn_session(&self.connect_profile_name, &self.connect_username, &self.connect_password, totp) {
                        Ok(message) => {
                            eprintln!("{}", message);
                        }
                        Err(error) => {
                            eprintln!("Failed to start session for profile '{}': {}", self.connect_profile_name, error);
                        }
                    }
                    // Always refresh the profiles list after connection attempt
                    if self.openvpn3_available {
                        self.openvpn_profiles = get_openvpn_profiles().unwrap_or_default();
                    }
                }
            }
            Message::SessionStarted(profile_name) => {
                eprintln!("Session started for profile: {}", profile_name);
                // Refresh the profiles list to show the new session status
                if self.openvpn3_available {
                    self.openvpn_profiles = get_openvpn_profiles().unwrap_or_default();
                }
            }
        }
        Task::none()
    }


    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

impl OpenVpn3Status {
    /// View for the import dialog
    fn view_import_dialog(&self, id: Id) -> Element<Message> {
        let file_path_input = widget::text_input("File path", &self.import_file_path)
            .on_input(Message::ImportFilePathChanged);
        
        let custom_name_input = widget::text_input("Custom name (optional)", &self.import_custom_name)
            .on_input(Message::ImportCustomNameChanged);
        
        let submit_button = widget::button::text("Import")
            .on_press(Message::ImportConfigSubmit);
        
        let cancel_button = widget::button::text("Cancel")
            .on_press(Message::ImportDialogClosed(id));
        
        let button_row = widget::row()
            .push(submit_button)
            .push(cancel_button)
            .spacing(10);
        
        let dialog_content = widget::column()
            .push(widget::text("Import OpenVPN Config"))
            .push(file_path_input)
            .push(custom_name_input)
            .push(button_row)
            .spacing(10)
            .padding(20);
        
        widget::container(dialog_content)
            .width(400)
            .height(200)
            .into()
    }
    
    /// View for the connect dialog
    fn view_connect_dialog(&self, id: Id) -> Element<Message> {
        let username_input = widget::text_input("Username", &self.connect_username)
            .on_input(Message::ConnectUsernameChanged);
        
        let password_input = widget::text_input("Password", &self.connect_password)
            .password()
            .on_input(Message::ConnectPasswordChanged);
        
        let totp_input = widget::text_input("TOTP (optional)", &self.connect_totp)
            .on_input(Message::ConnectTotpChanged);
        
        let connect_button = widget::button::text("Connect")
            .on_press(Message::ConnectSubmit);
        
        let cancel_button = widget::button::text("Cancel")
            .on_press(Message::ConnectDialogClosed(id));
        
        let button_row = widget::row()
            .push(connect_button)
            .push(cancel_button)
            .spacing(10);
        
        let dialog_content = widget::column()
            .push(widget::text("Connect to VPN"))
            .push(username_input)
            .push(password_input)
            .push(totp_input)
            .push(button_row)
            .spacing(10)
            .padding(20);
        
        widget::container(dialog_content)
            .width(400)
            .height(250)
            .into()
    }
}
