// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::Limits;
use cosmic::iced_winit::commands::popup::{destroy_popup, get_popup};
use cosmic::widget::{self, settings};
use cosmic::{Application, Element};

use crate::fl;
use crate::core::openvpn::{get_openvpn_profiles, is_openvpn3_available, delete_openvpn_profile, is_profile_session_active, disconnect_openvpn_session, OpenVPNProfile};

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
}

/// This is the enum that contains all the possible variants that your application will need to transmit messages.
/// This is used to communicate between the different parts of your application.
/// If your application does not need to send messages, you can use an empty enum or `()`.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    ToggleExampleRow(bool),
    DeleteProfile(String), // Profile name to delete
    DisconnectSession(String), // Profile name to disconnect
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
            .spacing(0)
            .add(settings::item(
                fl!("example-row"),
                widget::toggler(self.example_row).on_toggle(Message::ToggleExampleRow),
            ));

        // Add OpenVPN 3 profiles if available
        if self.openvpn3_available {
            if self.openvpn_profiles.is_empty() {
                content_list = content_list.add(settings::item(
                    "OpenVPN 3 Profiles",
                    widget::text("No profiles found"),
                ));
            } else {
                // Add header for profiles section
                content_list = content_list.add(settings::item(
                    "OpenVPN 3 Profiles",
                    widget::text(""),
                ));
                
                // Add each profile
                for profile in &self.openvpn_profiles {
                    let is_active = is_profile_session_active(&profile.name);
                    
                    if is_active {
                        // Show disconnect button for active sessions
                        let disconnect_button = widget::button::icon(widget::icon::from_name("media-playback-stop-symbolic"))
                            .on_press(Message::DisconnectSession(profile.name.clone()));
                        
                        content_list = content_list.add(settings::item(
                            &profile.name,
                            disconnect_button,
                        ));
                    } else {
                        // Show delete button for inactive profiles
                        let delete_button = widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                            .on_press(Message::DeleteProfile(profile.name.clone()));
                        
                        content_list = content_list.add(settings::item(
                            &profile.name,
                            delete_button,
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

        self.core.applet.popup_container(content_list).into()
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
            Message::ToggleExampleRow(toggled) => self.example_row = toggled,
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
                    }
                    Err(error) => {
                        // TODO: Show error message to user
                        eprintln!("Failed to disconnect session for profile '{}': {}", profile_name, error);
                    }
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}
