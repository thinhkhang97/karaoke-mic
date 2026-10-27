//! Which input and output device to use, given what is plugged in and what the
//! owner picked last time. Pure logic, so it can be tested without hardware.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
}

/// Preferred if present, else the first USB mic, else the default, else the first.
pub fn choose_input(available: &[DeviceInfo], preferred: Option<&str>) -> Option<String> {
    find_named(available, preferred)
        .or_else(|| available.iter().find(|d| d.name.to_lowercase().contains("usb")))
        .or_else(|| find_default_or_first(available))
        .map(|d| d.name.clone())
}

/// Preferred if present, else the default, else the first.
pub fn choose_output(available: &[DeviceInfo], preferred: Option<&str>) -> Option<String> {
    find_named(available, preferred)
        .or_else(|| find_default_or_first(available))
        .map(|d| d.name.clone())
}

fn find_named<'a>(available: &'a [DeviceInfo], name: Option<&str>) -> Option<&'a DeviceInfo> {
    name.and_then(|n| available.iter().find(|d| d.name == n))
}

fn find_default_or_first(available: &[DeviceInfo]) -> Option<&DeviceInfo> {
    available.iter().find(|d| d.is_default).or_else(|| available.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, is_default: bool) -> DeviceInfo {
        DeviceInfo { name: name.to_string(), is_default }
    }

    fn mics() -> Vec<DeviceInfo> {
        vec![dev("MacBook Pro Microphone", true), dev("ATR2100x-USB Microphone", false)]
    }

    #[test]
    fn input_prefers_saved() {
        assert_eq!(choose_input(&mics(), Some("MacBook Pro Microphone")).as_deref(), Some("MacBook Pro Microphone"));
    }

    #[test]
    fn input_falls_back_to_usb_then_default() {
        assert_eq!(choose_input(&mics(), None).as_deref(), Some("ATR2100x-USB Microphone"));
        let no_usb = vec![dev("Other", false), dev("MacBook Pro Microphone", true)];
        assert_eq!(choose_input(&no_usb, None).as_deref(), Some("MacBook Pro Microphone"));
        let no_default = vec![dev("First", false), dev("Second", false)];
        assert_eq!(choose_input(&no_default, None).as_deref(), Some("First"));
    }

    #[test]
    fn fallback_when_preferred_missing() {
        let only_builtin = vec![dev("MacBook Pro Microphone", true)];
        assert_eq!(choose_input(&only_builtin, Some("ATR2100x-USB Microphone")).as_deref(), Some("MacBook Pro Microphone"));
        let outs = vec![dev("MacBook Pro Speakers", true)];
        assert_eq!(choose_output(&outs, Some("External Headphones")).as_deref(), Some("MacBook Pro Speakers"));
    }

    #[test]
    fn returns_to_preferred_when_back() {
        let preferred = Some("ATR2100x-USB Microphone");
        let without = vec![dev("MacBook Pro Microphone", true)];
        assert_eq!(choose_input(&without, preferred).as_deref(), Some("MacBook Pro Microphone"));
        assert_eq!(choose_input(&mics(), preferred).as_deref(), Some("ATR2100x-USB Microphone"));
    }

    #[test]
    fn output_prefers_saved_then_default() {
        let outs = vec![dev("MacBook Pro Speakers", true), dev("External Headphones", false)];
        assert_eq!(choose_output(&outs, Some("External Headphones")).as_deref(), Some("External Headphones"));
        assert_eq!(choose_output(&outs, None).as_deref(), Some("MacBook Pro Speakers"));
    }

    #[test]
    fn empty_list_gives_none() {
        assert_eq!(choose_input(&[], Some("x")), None);
        assert_eq!(choose_output(&[], None), None);
    }
}
