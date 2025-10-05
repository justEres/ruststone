use super::*;

mod v1_8_9;

// https://wiki.vg/Protocol_History
// https://wiki.vg/Protocol_version_numbers#Versions_after_the_Netty_rewrite

pub fn protocol_name_to_protocol_version(s: String) -> i32 {
    match s.as_ref() {
        "" => SUPPORTED_PROTOCOLS[0],
        "1.16.5" => 754,
        "1.16.4" => 754,
        "1.16.3" => 753,
        "1.16.2" => 751,
        "1.16.1" => 736,
        "1.16" => 735,
        "1.15.2" => 578,
        "1.15.1" => 575,
        "1.14.4" => 498,
        "1.14.3" => 490,
        "1.14.2" => 485,
        "1.14.1" => 480,
        "1.14" => 477,
        "1.13.2" => 404,
        "1.12.2" => 340,
        "1.11.2" => 316,
        "1.11" => 315,
        "1.10.2" => 210,
        "1.9.2" => 109,
        "1.9" => 107,
        "1.8.9" => 47,
        "1.7.10" => 5,
        _ => {
            if let Ok(n) = s.parse::<i32>() {
                n
            } else {
                panic!("Unrecognized protocol name: {}", s)
            }
        }
    }
}

pub fn translate_internal_packet_id_for_version(
    version: i32,
    state: State,
    dir: Direction,
    id: i32,
    to_internal: bool,
) -> i32 {
    match version {
        47 => v1_8_9::translate_internal_packet_id(state, dir, id, to_internal),
        _ => panic!("unsupported protocol version: {}", version),
    }
}
