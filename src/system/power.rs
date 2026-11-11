use gtk4::gio;
use gtk4::gio::prelude::DBusProxyExt;
use gtk4::glib;
use gtk4::prelude::ToVariant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Suspend,
    Reboot,
    PowerOff,
}

impl PowerAction {
    fn dbus_method(self) -> &'static str {
        match self {
            PowerAction::Suspend => "Suspend",
            PowerAction::Reboot => "Reboot",
            PowerAction::PowerOff => "PowerOff",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PowerAction::Suspend => crate::i18n::STRINGS.suspend_label,
            PowerAction::Reboot => crate::i18n::STRINGS.reboot_label,
            PowerAction::PowerOff => crate::i18n::STRINGS.poweroff_label,
        }
    }
}

pub fn execute_power_action(action: PowerAction, mock: bool) -> Result<(), String> {
    if mock {
        return Err(format!(
            "Mock mode: '{}' skipped (not running under greetd).",
            action.dbus_method()
        ));
    }

    let proxy = gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::NONE,
        None::<&gio::DBusInterfaceInfo>,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        gio::Cancellable::NONE,
    )
    .map_err(|e| format!("Could not connect to systemd-logind: {e}"))?;

    let params = glib::Variant::tuple_from_iter([false.to_variant()]);
    proxy
        .call_sync(
            action.dbus_method(),
            Some(&params),
            gio::DBusCallFlags::NONE,
            5000,
            gio::Cancellable::NONE,
        )
        .map_err(|e| format!("Power action '{}' rejected: {e}", action.dbus_method()))?;

    Ok(())
}
