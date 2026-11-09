//! macOS: the `pbs` preferences that decide whether a Quick Action shows in Finder.
//!
//! Registering a workflow in `~/Library/Services` is not enough: Finder's
//! right-click menu only lists a service whose entry under `NSServicesStatus`
//! has `presentation_modes.ContextMenu = true`. Automator writes that entry when a
//! user saves a Quick Action; a third-party installer has to write it itself.
//! The domain is always accessed through `NSUserDefaults`, never by editing
//! `pbs.plist`, so cfprefsd and Finder see the change at once and nothing else in
//! the domain is touched.

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSDictionary, NSMutableDictionary, NSNumber, NSString, NSUserDefaults};

use crate::error::{Error, Result};
use crate::integrations::ACTION_NAME;

/// The preferences domain Finder reads service settings from.
pub const PBS_DOMAIN: &str = "pbs";
const STATUS_KEY: &str = "NSServicesStatus";
const MODES_KEY: &str = "presentation_modes";
const CONTEXT_MENU: &str = "ContextMenu";

/// How `pbs` names a workflow service: `(null) - <menu title> - <NSMessage>`.
pub fn quick_action_key() -> String {
    format!("(null) - {ACTION_NAME} - runWorkflowAsService")
}

fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn defaults(domain: &str) -> Result<Retained<NSUserDefaults>> {
    NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&ns(domain)))
        .ok_or_else(|| Error::Backend(format!("cannot open the {domain} preferences")))
}

/// What Automator writes for a Quick Action that is switched on. The values must be
/// real booleans: `pbs` silently ignores the strings "1"/"0".
fn enabled_entry() -> Retained<NSMutableDictionary<NSString, AnyObject>> {
    let entry = NSMutableDictionary::<NSString, AnyObject>::new();
    entry.insert(&*ns(MODES_KEY), &*enabled_modes());
    entry
}

fn enabled_modes() -> Retained<NSMutableDictionary<NSString, AnyObject>> {
    let modes = NSMutableDictionary::<NSString, AnyObject>::new();
    for (mode, on) in [
        (CONTEXT_MENU, true),
        ("FinderPreview", true),
        ("ServicesMenu", true),
        ("TouchBar", false),
    ] {
        modes.insert(&*ns(mode), &*NSNumber::new_bool(on));
    }
    modes
}

/// A mutable copy of the domain's `NSServicesStatus`, so every other service's
/// entry is written back unchanged.
fn status(
    defaults: &NSUserDefaults,
) -> (
    Option<Retained<AnyObject>>,
    Retained<NSMutableDictionary<NSString, AnyObject>>,
) {
    let existing = defaults.dictionaryForKey(&ns(STATUS_KEY));
    let ours = existing
        .as_ref()
        .and_then(|d| d.objectForKey(&ns(&quick_action_key())));
    let copy = match existing {
        Some(d) => NSMutableDictionary::dictionaryWithDictionary(&d),
        None => NSMutableDictionary::new(),
    };
    (ours, copy)
}

fn write_status(defaults: &NSUserDefaults, status: &NSDictionary<NSString, AnyObject>) {
    // SAFETY: a dictionary of strings, dictionaries and booleans is a valid
    // property-list value.
    unsafe { defaults.setObject_forKey(Some(status), &ns(STATUS_KEY)) };
    defaults.synchronize();
}

/// Marks the Quick Action as shown in Finder's right-click menu. Without `force`,
/// an entry that already exists is left alone, so a user who switched the action
/// off in System Settings keeps their choice.
pub fn enable_quick_action(domain: &str, force: bool) -> Result<()> {
    let defaults = defaults(domain)?;
    let (ours, status) = status(&defaults);
    if ours.is_some() && !force {
        return Ok(());
    }
    // Keep anything else in an existing entry, such as a keyboard shortcut.
    let entry = match ours.and_then(|o| o.downcast::<NSDictionary>().ok()) {
        Some(existing) => {
            // SAFETY: dictionaries read back from NSUserDefaults are property lists,
            // whose keys are always strings.
            let existing: Retained<NSDictionary<NSString, AnyObject>> =
                unsafe { Retained::cast_unchecked(existing) };
            let entry = NSMutableDictionary::dictionaryWithDictionary(&existing);
            entry.insert(&*ns(MODES_KEY), &*enabled_modes());
            entry
        }
        None => enabled_entry(),
    };
    status.insert(&*ns(&quick_action_key()), &*entry);
    write_status(&defaults, &status);
    Ok(())
}

/// Removes the Quick Action's entry; other services keep theirs.
pub fn remove_quick_action_status(domain: &str) -> Result<()> {
    let defaults = defaults(domain)?;
    let (ours, status) = status(&defaults);
    if ours.is_none() {
        return Ok(());
    }
    status.removeObjectForKey(&ns(&quick_action_key()));
    write_status(&defaults, &status);
    Ok(())
}

/// Whether the Quick Action is set to appear in Finder's right-click menu;
/// `None` when the domain has no entry for it.
pub fn quick_action_enabled(domain: &str) -> Option<bool> {
    let defaults = defaults(domain).ok()?;
    let (ours, _) = status(&defaults);
    let entry = ours?.downcast::<NSDictionary>().ok()?;
    let on = entry
        .objectForKey(&ns(MODES_KEY))
        .and_then(|m| m.downcast::<NSDictionary>().ok())
        .and_then(|m| m.objectForKey(&ns(CONTEXT_MENU)))
        .and_then(|v| v.downcast::<NSNumber>().ok())
        .is_some_and(|v| v.as_bool());
    Some(on)
}
