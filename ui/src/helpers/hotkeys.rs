use iced::keyboard::{key::Named, Key, Modifiers, key};
use smudgy_core::models::hotkeys::HotkeyDefinition;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaybePhysicalKey {
    Key(iced::keyboard::Key),
    Physical(iced::keyboard::key::Physical),
}

/// Efficient storage for hotkey data with pre-converted iced types
#[derive(Debug, Clone)]
pub struct HotkeyKeys {
    pub main_key: MaybePhysicalKey,
    pub modifiers: iced::keyboard::Modifiers,
}

impl From<HotkeyDefinition> for HotkeyKeys {
    fn from(hotkey: HotkeyDefinition) -> Self {
        let main_key = hotkey_to_maybe_physical_key(&hotkey);
        let modifiers = hotkey_to_iced_modifiers(&hotkey);
        
        HotkeyKeys {
            main_key,
            modifiers,
        }
    }
}

/// Converts a vector of MaybePhysicalKeys to strings and updates the hotkey definition
pub fn set_key_and_modifiers_from_maybe_physical(hotkey: &mut HotkeyDefinition, keys: Vec<MaybePhysicalKey>) {
    let mut key_out: Vec<String> = Vec::new();
    let mut modifiers: Vec<String> = Vec::new();

    for maybe_key in keys {
        match maybe_key {
            MaybePhysicalKey::Key(key) => {
                match key {
                    Key::Named(Named::Control) => {
                        modifiers.push("CTRL".to_string());
                    }
                    Key::Named(Named::Alt) => {
                        modifiers.push("ALT".to_string());
                    }
                    Key::Named(Named::Shift) => {
                        modifiers.push("SHIFT".to_string());
                    }
                    Key::Named(Named::Super) => {
                        modifiers.push("SUPER".to_string());
                    }
                    Key::Named(name) => {
                        key_out.push(format!("{:?}", name));
                    }
                    Key::Character(c) => {
                        key_out.push(format!("Character({})", c));
                    }
                    Key::Unidentified => {
                        key_out.push("UNIDENTIFIED".to_string());
                    }
                }
            }
            MaybePhysicalKey::Physical(physical) => {
                match physical {
                    key::Physical::Code(code) => {
                        key_out.push(format!("Code({:?})", code));
                    }
                    key::Physical::Unidentified(id) => {
                        key_out.push(format!("PhysicalUnidentified({:?})", id));
                    }
                }
            }
        }
    }

    hotkey.key = key_out.get(0).unwrap_or(&"UNIDENTIFIED".to_string()).clone();
    hotkey.modifiers = modifiers;
}

/// Converts a vector of iced Keys to strings and updates the hotkey definition
pub fn set_key_and_modifiers_from_iced(hotkey: &mut HotkeyDefinition, keys: Vec<Key>) {
    let mut key_out: Vec<String> = Vec::new();
    let mut modifiers: Vec<String> = Vec::new();

    for key in keys {
        match key {
            Key::Named(Named::Control) => {
                modifiers.push("CTRL".to_string());
            }
            Key::Named(Named::Alt) => {
                modifiers.push("ALT".to_string());
            }
            Key::Named(Named::Shift) => {
                modifiers.push("SHIFT".to_string());
            }
            Key::Named(Named::Super) => {
                modifiers.push("SUPER".to_string());
            }

            Key::Named(name) => {
                key_out.push(format!("{:?}", name));
            }
            Key::Character(c) => {
                key_out.push(c.to_string());
            }
            Key::Unidentified => {
                key_out.push("UNIDENTIFIED".to_string());
            }
        }
    }

    hotkey.key = key_out.get(0).unwrap_or(&"UNIDENTIFIED".to_string()).clone();
    hotkey.modifiers = modifiers;
}



/// Converts a hotkey definition to a MaybePhysicalKey (the primary key, not modifiers)
pub fn hotkey_to_maybe_physical_key(hotkey: &HotkeyDefinition) -> MaybePhysicalKey {
    // Check for physical key codes first
    if hotkey.key.starts_with("Code(") {
        if let Some(code_str) = hotkey.key.get(5..hotkey.key.len() - 1) {
            if let Some(code) = physical_code_from_str(code_str) {
                return MaybePhysicalKey::Physical(key::Physical::Code(code));
            }
        }
    }
    
    // Check for named logical keys
    if let Key::Named(named) = named_key_from_str(&hotkey.key) {
        return MaybePhysicalKey::Key(Key::Named(named));
    }
    
    // Check for character keys
    if hotkey.key.starts_with("Character(") {
        if let Some(c) = hotkey.key.get(10..hotkey.key.len() - 1) {
            return MaybePhysicalKey::Key(Key::Character(c.into()));
        }
    }
    
    // Fallback to unidentified
    MaybePhysicalKey::Key(Key::Unidentified)
}

/// Converts a hotkey definition to iced Modifiers
pub fn hotkey_to_iced_modifiers(hotkey: &HotkeyDefinition) -> Modifiers {
    hotkey.modifiers.iter().fold( Modifiers::empty(),|m, item| match item.as_str() {
        "CTRL" => m.union(Modifiers::CTRL),
        "ALT" => m.union(Modifiers::ALT),
        "SHIFT" => m.union(Modifiers::SHIFT),
        "SUPER" => m.union(Modifiers::LOGO),
        _ => m,
    })
}

/// Converts a string to an iced Physical Code by brute force iteration
fn physical_code_from_str(s: &str) -> Option<key::Code> {
    // Brute force through all possible Code variants
    let min = key::Code::Backquote as u8;
    let max = key::Code::F35 as u8;

    for i in min..=max {
        let code = unsafe { std::mem::transmute::<u8, key::Code>(i) };
        let code_str = format!("{:?}", code);
        if code_str == s {
            return Some(code);
        }
    }

    None
}

/// Converts a string to an iced Named Key by brute force iteration
fn named_key_from_str(s: &str) -> Key {
    // Let's brute force it

    let min = Named::Alt as u16;
    let max = Named::F35 as u16;

    for i in min..=max {
        let i_as_key = unsafe { std::mem::transmute::<u16, Named>(i) };
        let key_str = format!("{:?}", i_as_key);
        if key_str == s {
            return Key::Named(i_as_key);
        }
    }

    Key::Unidentified
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::{key::Named, Key};

    #[test]
    fn test_named_key_from_str_modifier_keys() {
        // Test modifier keys
        assert_eq!(named_key_from_str("Alt"), Key::Named(Named::Alt));
        assert_eq!(named_key_from_str("Control"), Key::Named(Named::Control));
        assert_eq!(named_key_from_str("Shift"), Key::Named(Named::Shift));
        assert_eq!(named_key_from_str("Super"), Key::Named(Named::Super));
        assert_eq!(named_key_from_str("Meta"), Key::Named(Named::Meta));
        assert_eq!(named_key_from_str("AltGraph"), Key::Named(Named::AltGraph));
    }

    #[test]
    fn test_named_key_from_str_navigation_keys() {
        // Test navigation keys
        assert_eq!(named_key_from_str("Enter"), Key::Named(Named::Enter));
        assert_eq!(named_key_from_str("Tab"), Key::Named(Named::Tab));
        assert_eq!(named_key_from_str("Space"), Key::Named(Named::Space));
        assert_eq!(named_key_from_str("Escape"), Key::Named(Named::Escape));
        assert_eq!(named_key_from_str("Backspace"), Key::Named(Named::Backspace));
        assert_eq!(named_key_from_str("Delete"), Key::Named(Named::Delete));
    }

    #[test]
    fn test_named_key_from_str_arrow_keys() {
        // Test arrow keys
        assert_eq!(named_key_from_str("ArrowUp"), Key::Named(Named::ArrowUp));
        assert_eq!(named_key_from_str("ArrowDown"), Key::Named(Named::ArrowDown));
        assert_eq!(named_key_from_str("ArrowLeft"), Key::Named(Named::ArrowLeft));
        assert_eq!(named_key_from_str("ArrowRight"), Key::Named(Named::ArrowRight));
    }

    #[test]
    fn test_named_key_from_str_home_end_keys() {
        // Test Home/End/PageUp/PageDown keys
        assert_eq!(named_key_from_str("Home"), Key::Named(Named::Home));
        assert_eq!(named_key_from_str("End"), Key::Named(Named::End));
        assert_eq!(named_key_from_str("PageUp"), Key::Named(Named::PageUp));
        assert_eq!(named_key_from_str("PageDown"), Key::Named(Named::PageDown));
    }

    #[test]
    fn test_named_key_from_str_function_keys() {
        // Test function keys F1-F12
        assert_eq!(named_key_from_str("F1"), Key::Named(Named::F1));
        assert_eq!(named_key_from_str("F2"), Key::Named(Named::F2));
        assert_eq!(named_key_from_str("F3"), Key::Named(Named::F3));
        assert_eq!(named_key_from_str("F4"), Key::Named(Named::F4));
        assert_eq!(named_key_from_str("F5"), Key::Named(Named::F5));
        assert_eq!(named_key_from_str("F6"), Key::Named(Named::F6));
        assert_eq!(named_key_from_str("F7"), Key::Named(Named::F7));
        assert_eq!(named_key_from_str("F8"), Key::Named(Named::F8));
        assert_eq!(named_key_from_str("F9"), Key::Named(Named::F9));
        assert_eq!(named_key_from_str("F10"), Key::Named(Named::F10));
        assert_eq!(named_key_from_str("F11"), Key::Named(Named::F11));
        assert_eq!(named_key_from_str("F12"), Key::Named(Named::F12));
    }

    #[test]
    fn test_named_key_from_str_extended_function_keys() {
        // Test extended function keys F13-F35
        assert_eq!(named_key_from_str("F13"), Key::Named(Named::F13));
        assert_eq!(named_key_from_str("F20"), Key::Named(Named::F20));
        assert_eq!(named_key_from_str("F24"), Key::Named(Named::F24));
        assert_eq!(named_key_from_str("F30"), Key::Named(Named::F30));
        assert_eq!(named_key_from_str("F35"), Key::Named(Named::F35));
    }

    #[test]
    fn test_named_key_from_str_lock_keys() {
        // Test lock keys
        assert_eq!(named_key_from_str("CapsLock"), Key::Named(Named::CapsLock));
        assert_eq!(named_key_from_str("NumLock"), Key::Named(Named::NumLock));
        assert_eq!(named_key_from_str("ScrollLock"), Key::Named(Named::ScrollLock));
    }

    #[test]
    fn test_named_key_from_str_editing_keys() {
        // Test editing keys
        assert_eq!(named_key_from_str("Insert"), Key::Named(Named::Insert));
        assert_eq!(named_key_from_str("Copy"), Key::Named(Named::Copy));
        assert_eq!(named_key_from_str("Cut"), Key::Named(Named::Cut));
        assert_eq!(named_key_from_str("Paste"), Key::Named(Named::Paste));
        assert_eq!(named_key_from_str("Undo"), Key::Named(Named::Undo));
        assert_eq!(named_key_from_str("Redo"), Key::Named(Named::Redo));
    }

    #[test]
    fn test_named_key_from_str_special_keys() {
        // Test special keys
        assert_eq!(named_key_from_str("PrintScreen"), Key::Named(Named::PrintScreen));
        assert_eq!(named_key_from_str("Pause"), Key::Named(Named::Pause));
        assert_eq!(named_key_from_str("ContextMenu"), Key::Named(Named::ContextMenu));
        assert_eq!(named_key_from_str("Help"), Key::Named(Named::Help));
    }

    #[test]
    fn test_named_key_from_str_media_keys() {
        // Test media keys
        assert_eq!(named_key_from_str("MediaPlay"), Key::Named(Named::MediaPlay));
        assert_eq!(named_key_from_str("MediaPause"), Key::Named(Named::MediaPause));
        assert_eq!(named_key_from_str("MediaPlayPause"), Key::Named(Named::MediaPlayPause));
        assert_eq!(named_key_from_str("MediaStop"), Key::Named(Named::MediaStop));
        assert_eq!(named_key_from_str("MediaTrackNext"), Key::Named(Named::MediaTrackNext));
        assert_eq!(named_key_from_str("MediaTrackPrevious"), Key::Named(Named::MediaTrackPrevious));
    }

    #[test]
    fn test_named_key_from_str_browser_keys() {
        // Test browser keys
        assert_eq!(named_key_from_str("BrowserBack"), Key::Named(Named::BrowserBack));
        assert_eq!(named_key_from_str("BrowserForward"), Key::Named(Named::BrowserForward));
        assert_eq!(named_key_from_str("BrowserRefresh"), Key::Named(Named::BrowserRefresh));
        assert_eq!(named_key_from_str("BrowserHome"), Key::Named(Named::BrowserHome));
        assert_eq!(named_key_from_str("BrowserSearch"), Key::Named(Named::BrowserSearch));
        assert_eq!(named_key_from_str("BrowserFavorites"), Key::Named(Named::BrowserFavorites));
        assert_eq!(named_key_from_str("BrowserStop"), Key::Named(Named::BrowserStop));
    }

    #[test]
    fn test_named_key_from_str_invalid_keys() {
        // Test invalid/unrecognized key names
        assert_eq!(named_key_from_str("InvalidKey"), Key::Unidentified);
        assert_eq!(named_key_from_str(""), Key::Unidentified);
        assert_eq!(named_key_from_str("123"), Key::Unidentified);
        assert_eq!(named_key_from_str("!@#$"), Key::Unidentified);
        assert_eq!(named_key_from_str("NotAKey"), Key::Unidentified);
    }

    #[test]
    fn test_named_key_from_str_case_sensitivity() {
        // Test case sensitivity - the function is case-sensitive
        assert_eq!(named_key_from_str("enter"), Key::Unidentified); // lowercase
        assert_eq!(named_key_from_str("ENTER"), Key::Unidentified); // uppercase
        assert_eq!(named_key_from_str("Enter"), Key::Named(Named::Enter)); // correct case
        
        assert_eq!(named_key_from_str("space"), Key::Unidentified); // lowercase
        assert_eq!(named_key_from_str("SPACE"), Key::Unidentified); // uppercase
        assert_eq!(named_key_from_str("Space"), Key::Named(Named::Space)); // correct case
    }

    #[test]
    fn test_named_key_from_str_edge_cases() {
        // Test edge cases
        assert_eq!(named_key_from_str("F0"), Key::Unidentified); // F0 doesn't exist
        assert_eq!(named_key_from_str("F36"), Key::Unidentified); // Beyond F35
        assert_eq!(named_key_from_str("F1000"), Key::Unidentified); // Way beyond range
        
        // Test with extra spaces (should not match)
        assert_eq!(named_key_from_str(" Enter"), Key::Unidentified);
        assert_eq!(named_key_from_str("Enter "), Key::Unidentified);
        assert_eq!(named_key_from_str(" Enter "), Key::Unidentified);
    }

    #[test]
    fn test_named_key_from_str_less_common_keys() {
        // Test less common but valid keys
        assert_eq!(named_key_from_str("Fn"), Key::Named(Named::Fn));
        assert_eq!(named_key_from_str("FnLock"), Key::Named(Named::FnLock));
        assert_eq!(named_key_from_str("Hyper"), Key::Named(Named::Hyper));
        assert_eq!(named_key_from_str("Symbol"), Key::Named(Named::Symbol));
        assert_eq!(named_key_from_str("SymbolLock"), Key::Named(Named::SymbolLock));
        assert_eq!(named_key_from_str("Clear"), Key::Named(Named::Clear));
        assert_eq!(named_key_from_str("Execute"), Key::Named(Named::Execute));
        assert_eq!(named_key_from_str("Select"), Key::Named(Named::Select));
        assert_eq!(named_key_from_str("Find"), Key::Named(Named::Find));
        assert_eq!(named_key_from_str("Again"), Key::Named(Named::Again));
        assert_eq!(named_key_from_str("Props"), Key::Named(Named::Props));
        assert_eq!(named_key_from_str("ZoomIn"), Key::Named(Named::ZoomIn));
        assert_eq!(named_key_from_str("ZoomOut"), Key::Named(Named::ZoomOut));
    }

    #[test]
    fn test_set_key_and_modifiers_from_iced() {
        let mut hotkey = HotkeyDefinition {
            key: "".to_string(),
            modifiers: vec![],
            script: None,
            package: None,
            language: smudgy_core::models::ScriptLang::Plaintext,
            enabled: true,
        };

        let keys = vec![
            Key::Named(Named::Control),
            Key::Named(Named::Shift),
            Key::Character("a".into())
        ];

        set_key_and_modifiers_from_iced(&mut hotkey, keys);

        assert_eq!(hotkey.key, "a");
        assert_eq!(hotkey.modifiers, vec!["CTRL", "SHIFT"]);
    }

    #[test]
    fn test_hotkey_to_maybe_physical_key() {
        let hotkey = HotkeyDefinition {
            key: "Space".to_string(),
            modifiers: vec!["CTRL".to_string(), "ALT".to_string()],
            script: None,
            package: None,
            language: smudgy_core::models::ScriptLang::Plaintext,
            enabled: true,
        };

        let maybe_key = hotkey_to_maybe_physical_key(&hotkey);
        
        match maybe_key {
            MaybePhysicalKey::Key(Key::Named(Named::Space)) => {
                // Expected result
            }
            _ => panic!("Expected MaybePhysicalKey::Key(Key::Named(Named::Space))"),
        }
    }

    #[test]
    fn test_from_hotkey_definition() {
        let hotkey = HotkeyDefinition {
            key: "Enter".to_string(),
            modifiers: vec!["CTRL".to_string(), "SHIFT".to_string()],
            script: None,
            package: None,
            language: smudgy_core::models::ScriptLang::Plaintext,
            enabled: true,
        };

        let hotkey_keys: HotkeyKeys = hotkey.into();
        
        // Test the main key
        match hotkey_keys.main_key {
            MaybePhysicalKey::Key(Key::Named(Named::Enter)) => {
                // Expected result
            }
            _ => panic!("Expected MaybePhysicalKey::Key(Key::Named(Named::Enter))"),
        }
        
        // Test the modifiers
        assert!(hotkey_keys.modifiers.contains(Modifiers::CTRL));
        assert!(hotkey_keys.modifiers.contains(Modifiers::SHIFT));
        assert!(!hotkey_keys.modifiers.contains(Modifiers::ALT));
    }

    #[test]
    fn test_hotkey_to_maybe_physical_key_character() {
        let hotkey = HotkeyDefinition {
            key: "Character(a)".to_string(),
            modifiers: vec![],
            script: None,
            package: None,
            language: smudgy_core::models::ScriptLang::Plaintext,
            enabled: true,
        };

        let maybe_key = hotkey_to_maybe_physical_key(&hotkey);
        
        match maybe_key {
            MaybePhysicalKey::Key(Key::Character(c)) if c.as_str() == "a" => {
                // Expected result
            }
            _ => panic!("Expected MaybePhysicalKey::Key(Key::Character('a'))"),
        }
    }

    #[test]
    fn test_hotkey_to_maybe_physical_key_code() {
        let hotkey = HotkeyDefinition {
            key: "Code(KeyA)".to_string(),
            modifiers: vec![],
            script: None,
            package: None,
            language: smudgy_core::models::ScriptLang::Plaintext,
            enabled: true,
        };

        let maybe_key = hotkey_to_maybe_physical_key(&hotkey);
        
        match maybe_key {
            MaybePhysicalKey::Physical(key::Physical::Code(key::Code::KeyA)) => {
                // Expected result
            }
            _ => panic!("Expected MaybePhysicalKey::Physical(key::Physical::Code(key::Code::KeyA))"),
        }
    }

    #[test]
    fn test_hotkey_to_maybe_physical_key_code_f1() {
        let hotkey = HotkeyDefinition {
            key: "Code(F1)".to_string(),
            modifiers: vec![],
            script: None,
            package: None,
            language: smudgy_core::models::ScriptLang::Plaintext,
            enabled: true,
        };

        let maybe_key = hotkey_to_maybe_physical_key(&hotkey);
        
        match maybe_key {
            MaybePhysicalKey::Physical(key::Physical::Code(key::Code::F1)) => {
                // Expected result
            }
            _ => panic!("Expected MaybePhysicalKey::Physical(key::Physical::Code(key::Code::F1))"),
        }
    }

    #[test]
    fn test_hotkey_to_maybe_physical_key_invalid_code() {
        let hotkey = HotkeyDefinition {
            key: "Code(InvalidCode)".to_string(),
            modifiers: vec![],
            script: None,
            package: None,
            language: smudgy_core::models::ScriptLang::Plaintext,
            enabled: true,
        };

        let maybe_key = hotkey_to_maybe_physical_key(&hotkey);
        
        match maybe_key {
            MaybePhysicalKey::Key(Key::Unidentified) => {
                // Expected result - should fall back to unidentified
            }
            _ => panic!("Expected MaybePhysicalKey::Key(Key::Unidentified) for invalid code"),
        }
    }

    #[test]
    fn test_physical_code_from_str() {
        // Test valid codes
        assert_eq!(physical_code_from_str("KeyA"), Some(key::Code::KeyA));
        assert_eq!(physical_code_from_str("KeyZ"), Some(key::Code::KeyZ));
        assert_eq!(physical_code_from_str("F1"), Some(key::Code::F1));
        assert_eq!(physical_code_from_str("Enter"), Some(key::Code::Enter));
        assert_eq!(physical_code_from_str("Space"), Some(key::Code::Space));
        
        // Test invalid codes
        assert_eq!(physical_code_from_str("InvalidKey"), None);
        assert_eq!(physical_code_from_str(""), None);
        assert_eq!(physical_code_from_str("NotAKey"), None);
    }
} 