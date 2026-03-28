use super::*;

pub(super) fn handle_packet(pkt: Packet, to_main: &crossbeam::channel::Sender<FromNetMessage>) {
    match pkt {
        Packet::Title(title) => {
            send_title_packet(
                to_main,
                title.action.0,
                title.title.as_ref(),
                title.sub_title.as_ref(),
                title.action_bar_text.as_deref(),
                title.fade_in,
                title.fade_stay,
                title.fade_out,
            );
        }
        Packet::Title_notext(title) => match title.action.0 {
            0 => {
                if let Some(title) = title.title.as_ref() {
                    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetTitle {
                        text: component_to_legacy(title),
                    }));
                }
            }
            1 => {
                if let Some(subtitle) = title.sub_title.as_ref() {
                    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetSubtitle {
                        text: component_to_legacy(subtitle),
                    }));
                }
            }
            2 => send_title_times(to_main, title.fade_in, title.fade_stay, title.fade_out),
            3 => {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::Clear));
            }
            4 => {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::Reset));
            }
            _ => {}
        },
        Packet::Title_notext_component(title) => match title.action.0 {
            0 => {
                if let Some(title) = title.title.as_ref() {
                    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetTitle {
                        text: component_to_legacy(title),
                    }));
                }
            }
            1 => {
                if let Some(subtitle) = title.sub_title.as_ref() {
                    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetSubtitle {
                        text: component_to_legacy(subtitle),
                    }));
                }
            }
            3 => {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::Clear));
            }
            4 => {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::Reset));
            }
            _ => {}
        },
        _ => {}
    }
}
