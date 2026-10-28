//! Calls: is there a SIM and a mobile network, do calls go over 4G/5G (VoLTE - faster to connect, clearer
//! sound) or fall back to 2G/3G, and does the carrier offer VoLTE and Wi-Fi calling. The VoLTE switch itself
//! is closed to adb, but a carrier that offers VoLTE while calls run over 2G/3G means the switch is off.

use crate::phone::adb::Adb;
use crate::phone::info::DeviceInfo;
use crate::ui;

/// Radio technologies (ServiceState RIL_RADIO_TECHNOLOGY_*) that carry voice as VoLTE or VoNR.
const VOLTE_RADIOS: [u32; 3] = [14, 19, 20];
/// Wi-Fi calling.
const IWLAN: u32 = 18;

/// One voice service state from `dumpsys telephony.registry`.
pub struct Voice {
    pub in_service: bool,
    pub radio: u32,
}

/// Every "mServiceState={...}" entry: is voice in service, and over which radio technology.
pub fn parse_voice(registry: &str) -> Vec<Voice> {
    registry
        .split("mServiceState={")
        .skip(1)
        .map(|entry| Voice {
            in_service: entry.contains("mVoiceRegState=0(IN_SERVICE)"),
            radio: entry
                .split("getRilVoiceRadioTechnology=")
                .nth(1)
                .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok())
                .unwrap_or(0),
        })
        .collect()
}

/// A boolean carrier setting from `dumpsys carrier_config` ("carrier_volte_available_bool = true").
pub fn carrier_flag(config: &str, key: &str) -> bool {
    config.lines().any(|l| l.trim().strip_prefix(key).is_some_and(|rest| rest.trim() == "= true"))
}

/// `gsm.sim.state` lists every slot ("READY,ABSENT"); any slot other than absent holds a SIM.
pub fn sim_present(sim_state: &str) -> bool {
    sim_state.split(',').map(str::trim).any(|s| !s.is_empty() && s != "ABSENT" && s != "NOT_READY")
}

#[derive(Debug, PartialEq, Eq)]
pub enum Calls {
    NoSim,
    NoNetwork,
    OverVolte,
    OverWifi,
    /// The carrier offers VoLTE, yet calls run over 2G/3G: the switch is off.
    FallsBackVolteOff,
    /// The carrier does not offer VoLTE: 2G/3G calls are all there is.
    FallsBackNoVolte,
}

pub fn assess(sim: bool, voice: &[Voice], volte_offered: bool) -> Calls {
    if !sim {
        return Calls::NoSim;
    }
    let Some(serving) = voice.iter().find(|v| v.in_service) else { return Calls::NoNetwork };
    match serving.radio {
        r if VOLTE_RADIOS.contains(&r) => Calls::OverVolte,
        IWLAN => Calls::OverWifi,
        _ if volte_offered => Calls::FallsBackVolteOff,
        _ => Calls::FallsBackNoVolte,
    }
}

pub fn show(adb: &Adb, info: &DeviceInfo) {
    let sim = sim_present(&adb.sh("getprop gsm.sim.state"));
    let config = adb.sh("dumpsys carrier_config");
    let volte = carrier_flag(&config, "carrier_volte_available_bool");
    let wifi_calling = carrier_flag(&config, "carrier_wfc_ims_available_bool");
    ui::step("Calls");
    match assess(sim, &parse_voice(&adb.sh("dumpsys telephony.registry")), volte) {
        Calls::NoSim => ui::dim("No SIM card - calls not checked."),
        Calls::NoNetwork => ui::warn("No mobile network for calls right now (check the signal or the SIM)."),
        Calls::OverVolte => ui::ok("Calls go over 4G/5G (VoLTE): they connect fast and sound clear"),
        Calls::OverWifi => ui::ok("Calls go over Wi-Fi (Wi-Fi calling)"),
        Calls::FallsBackVolteOff => {
            ui::warn("Calls drop to 2G/3G although your carrier offers VoLTE - they connect slower and sound worse.");
            ui::dim(&format!("Turn VoLTE on: {}.", info.brand.volte_setting()));
        }
        Calls::FallsBackNoVolte => ui::dim("Calls go over 2G/3G - your carrier does not offer VoLTE on this SIM."),
    }
    if sim && wifi_calling {
        ui::dim("Weak signal at home? Your carrier offers Wi-Fi calling - turn it on next to VoLTE.");
    }
}
