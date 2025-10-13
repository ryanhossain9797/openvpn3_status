use app::OpenVpn3Status;

mod app;
mod core;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<OpenVpn3Status>(())
}
