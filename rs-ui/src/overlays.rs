use super::*;
use crate::state::{ChatAutocompleteState, ConnectUiState};

pub(crate) fn draw_chat_message(ui: &mut egui::Ui, msg: &str, font_size: f32) {
    let segments = parse_legacy_chat_segments(msg);
    ui.horizontal_wrapped(|ui| {
        for segment in segments {
            let mut rich = egui::RichText::new(segment.text)
                .color(segment.color)
                .size(font_size);
            if segment.bold {
                rich = rich.strong();
            }
            if segment.italic {
                rich = rich.italics();
            }
            if segment.underlined {
                rich = rich.underline();
            }
            if segment.strikethrough {
                rich = rich.strikethrough();
            }
            ui.label(rich);
        }
    });
}

pub(crate) fn handle_chat_tab_complete(
    to_net: &ToNet,
    chat: &mut Chat,
    chat_autocomplete: &mut ChatAutocompleteState,
) {
    if chat_autocomplete.suggestions.is_empty() {
        let query = chat.1.clone();
        if chat_autocomplete.pending_query.as_deref() != Some(query.as_str()) {
            let _ = to_net.0.send(ToNetMessage::TabCompleteRequest {
                text: query.clone(),
            });
            chat_autocomplete.pending_query = Some(query);
        }
        return;
    }

    let len = chat_autocomplete.suggestions.len();
    let idx = chat_autocomplete.selected.min(len - 1);
    let mut completion = chat_autocomplete.suggestions[idx].trim().to_string();
    if chat.1.starts_with('/') && !completion.starts_with('/') {
        completion.insert(0, '/');
    }
    chat.1 = completion;
    chat_autocomplete.query_snapshot = chat.1.clone();
    chat_autocomplete.suppress_next_clear = true;
    chat_autocomplete.selected = (idx + 1) % len;
}

pub(crate) fn draw_scoreboard_sidebar(
    ctx: &egui::Context,
    scoreboard: &ScoreboardState,
    state: &ConnectUiState,
) {
    let Some((_, objective)) = scoreboard.sidebar_objective() else {
        return;
    };
    let mut lines = scoreboard.sidebar_lines();
    if lines.is_empty() {
        return;
    }
    let title_text = single_line_plain_text(objective.display_name.as_str());

    egui::Area::new(egui::Id::new("scoreboard_sidebar"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .interactable(false)
        .show(ctx, |ui| {
            let font_id = egui::FontId::proportional(state.scoreboard_font_size);
            let bg_alpha = alpha_to_u8(state.scoreboard_background_opacity);
            let row_bg = egui::Color32::from_black_alpha(bg_alpha);
            let title_bg = egui::Color32::from_black_alpha(bg_alpha.saturating_add(24));
            let name_color = egui::Color32::from_gray(235);
            let score_color = egui::Color32::from_rgb(255, 85, 85);
            let title_color = egui::Color32::WHITE;

            let measure_width = |text: &str| -> f32 {
                ui.painter()
                    .layout_no_wrap(text.to_string(), font_id.clone(), egui::Color32::WHITE)
                    .size()
                    .x
            };

            let content_lines: Vec<(String, String)> = lines
                .drain(..)
                .map(|(name, value)| (single_line_plain_text(name.as_str()), value.to_string()))
                .collect();

            let mut max_width = measure_width(title_text.as_str());
            for (name, score) in &content_lines {
                let combined = if name.is_empty() {
                    score.clone()
                } else {
                    format!("{name}: {score}")
                };
                max_width = max_width.max(measure_width(combined.as_str()));
            }

            let row_height = ui
                .painter()
                .layout_no_wrap("Ay".to_string(), font_id.clone(), egui::Color32::WHITE)
                .size()
                .y
                .ceil();
            let screen_rect = ctx.screen_rect();
            let sidebar_width = (max_width + 6.0).clamp(48.0, screen_rect.width() * 0.4);
            let right = screen_rect.right() - 3.0;
            let total_rows = content_lines.len() as f32;
            let bottom = screen_rect.center().y + (total_rows * row_height) / 3.0;
            let left = right - sidebar_width;
            let painter = ui.painter();

            for (idx, (name, score)) in content_lines.iter().enumerate() {
                let top = bottom - (idx as f32 + 1.0) * row_height;
                let rect = egui::Rect::from_min_max(
                    egui::pos2(left - 2.0, top),
                    egui::pos2(right, top + row_height),
                );
                painter.rect_filled(rect, 0.0, row_bg);
                if !name.is_empty() {
                    painter.text(
                        egui::pos2(left, top + row_height * 0.5),
                        egui::Align2::LEFT_CENTER,
                        name,
                        font_id.clone(),
                        name_color,
                    );
                }
                painter.text(
                    egui::pos2(right - 2.0, top + row_height * 0.5),
                    egui::Align2::RIGHT_CENTER,
                    score,
                    font_id.clone(),
                    score_color,
                );
            }

            let title_top = bottom - (total_rows + 1.0) * row_height;
            let title_rect = egui::Rect::from_min_max(
                egui::pos2(left - 2.0, title_top - 1.0),
                egui::pos2(right, title_top + row_height - 1.0),
            );
            painter.rect_filled(title_rect, 0.0, title_bg);
            let separator_rect = egui::Rect::from_min_max(
                egui::pos2(left - 2.0, bottom - total_rows * row_height - 1.0),
                egui::pos2(right, bottom - total_rows * row_height),
            );
            painter.rect_filled(separator_rect, 0.0, row_bg);
            painter.text(
                egui::pos2(
                    (left + right) * 0.5 - 1.0,
                    title_top + row_height * 0.5 - 1.0,
                ),
                egui::Align2::CENTER_CENTER,
                title_text,
                font_id,
                title_color,
            );
        });
}

pub(crate) fn draw_title_overlay(
    ctx: &egui::Context,
    title_overlay: &TitleOverlayState,
    state: &ConnectUiState,
) {
    let Some(alpha) = overlay_alpha(title_overlay.title_started_at, title_overlay.times) else {
        return;
    };
    if title_overlay.title.is_empty() && title_overlay.subtitle.is_empty() {
        return;
    }

    let alpha_u8 = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    egui::Area::new(egui::Id::new("title_overlay"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -90.0))
        .interactable(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(
                    ((alpha * state.title_background_opacity).round()).clamp(0.0, 255.0) as u8,
                ))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .corner_radius(6.0);
            frame.show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    if !title_overlay.title.is_empty() {
                        let title = strip_legacy_codes(title_overlay.title.as_str());
                        ui.label(
                            egui::RichText::new(title)
                                .size(state.title_font_size)
                                .strong()
                                .color(egui::Color32::from_white_alpha(alpha_u8)),
                        );
                    }
                    if !title_overlay.subtitle.is_empty() {
                        let subtitle = strip_legacy_codes(title_overlay.subtitle.as_str());
                        ui.label(
                            egui::RichText::new(subtitle)
                                .size((state.title_font_size * 0.58).clamp(10.0, 36.0))
                                .color(egui::Color32::from_white_alpha(alpha_u8)),
                        );
                    }
                });
            });
        });
}

pub(crate) fn draw_action_bar_overlay(
    ctx: &egui::Context,
    title_overlay: &TitleOverlayState,
    state: &ConnectUiState,
) {
    if title_overlay.action_bar.is_empty() {
        return;
    }
    let Some(alpha) = overlay_alpha(title_overlay.action_bar_started_at, title_overlay.times)
    else {
        return;
    };

    let alpha_u8 = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    egui::Area::new(egui::Id::new("action_bar_overlay"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -86.0))
        .interactable(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(
                    ((alpha * state.title_background_opacity).round()).clamp(0.0, 255.0) as u8,
                ))
                .inner_margin(egui::Margin::same(6))
                .corner_radius(4.0);
            frame.show(ui, |ui| {
                let text = strip_legacy_codes(title_overlay.action_bar.as_str());
                ui.label(
                    egui::RichText::new(text)
                        .size((state.title_font_size * 0.53).clamp(10.0, 30.0))
                        .color(egui::Color32::from_white_alpha(alpha_u8)),
                );
            });
        });
}

pub(crate) fn draw_tab_list_overlay(
    ctx: &egui::Context,
    tab_list_header_footer: &TabListHeaderFooter,
    state: &ConnectUiState,
) {
    if tab_list_header_footer.header.trim().is_empty()
        && tab_list_header_footer.footer.trim().is_empty()
    {
        return;
    }

    egui::Area::new(egui::Id::new("tab_list_overlay"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 20.0))
        .interactable(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(alpha_to_u8(
                    state.scoreboard_background_opacity,
                )))
                .inner_margin(egui::Margin::same(10))
                .corner_radius(4.0);
            frame.show(ui, |ui| {
                ui.set_width(320.0);
                if !tab_list_header_footer.header.trim().is_empty() {
                    draw_legacy_text(
                        ui,
                        tab_list_header_footer.header.as_str(),
                        true,
                        state.scoreboard_font_size,
                    );
                }
                if !tab_list_header_footer.footer.trim().is_empty() {
                    if !tab_list_header_footer.header.trim().is_empty() {
                        ui.add_space(8.0);
                    }
                    draw_legacy_text(
                        ui,
                        tab_list_header_footer.footer.as_str(),
                        false,
                        state.scoreboard_font_size,
                    );
                }
            });
        });
}

fn overlay_alpha(
    started_at: Option<std::time::Instant>,
    times: rs_utils::TitleTimes,
) -> Option<f32> {
    let started_at = started_at?;
    let elapsed_ticks = started_at.elapsed().as_secs_f32() / 0.05;
    let fade_in = times.fade_in_ticks.max(0) as f32;
    let stay = times.stay_ticks.max(0) as f32;
    let fade_out = times.fade_out_ticks.max(0) as f32;
    let total = fade_in + stay + fade_out;
    if elapsed_ticks >= total.max(1.0) {
        return None;
    }
    if fade_in > 0.0 && elapsed_ticks < fade_in {
        return Some((elapsed_ticks / fade_in).clamp(0.0, 1.0));
    }
    if elapsed_ticks < fade_in + stay {
        return Some(1.0);
    }
    if fade_out > 0.0 {
        let fade_elapsed = elapsed_ticks - fade_in - stay;
        return Some((1.0 - fade_elapsed / fade_out).clamp(0.0, 1.0));
    }
    Some(1.0)
}

pub(crate) fn draw_legacy_text(ui: &mut egui::Ui, text: &str, centered: bool, font_size: f32) {
    let segments = parse_legacy_chat_segments(text);
    let layout_job = legacy_layout_job(&segments, font_size);
    if centered {
        ui.horizontal_centered(|ui| {
            ui.label(layout_job.clone());
        });
    } else {
        ui.label(layout_job);
    }
}

fn legacy_layout_job(segments: &[ChatSegment], font_size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for segment in segments {
        if segment.text.is_empty() {
            continue;
        }
        let mut format = egui::TextFormat {
            color: segment.color,
            font_id: egui::FontId::proportional(font_size),
            ..Default::default()
        };
        if segment.bold {
            format.font_id = egui::FontId::proportional(font_size);
        }
        if segment.italic {
            format.italics = true;
        }
        if segment.underlined {
            format.underline = egui::Stroke::new(1.0, segment.color);
        }
        if segment.strikethrough {
            format.strikethrough = egui::Stroke::new(1.0, segment.color);
        }
        job.append(segment.text.as_str(), 0.0, format);
    }
    job
}

pub(crate) fn alpha_to_u8(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn strip_legacy_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '§' {
            let _ = chars.next();
            continue;
        }
        out.push(ch);
    }
    out
}

fn single_line_plain_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in strip_legacy_codes(text).chars() {
        if ch.is_control() {
            pending_space = true;
            continue;
        }
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.push(ch);
    }
    out
}

#[derive(Clone)]
struct ChatSegment {
    text: String,
    color: egui::Color32,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
}

fn parse_legacy_chat_segments(msg: &str) -> Vec<ChatSegment> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut color = egui::Color32::from_rgb(230, 230, 230);
    let mut bold = false;
    let mut italic = false;
    let mut underlined = false;
    let mut strikethrough = false;

    let mut chars = msg.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '§' {
            buf.push(ch);
            continue;
        }
        let Some(code) = chars.next() else {
            buf.push(ch);
            break;
        };
        if !buf.is_empty() {
            out.push(ChatSegment {
                text: std::mem::take(&mut buf),
                color,
                bold,
                italic,
                underlined,
                strikethrough,
            });
        }
        match code.to_ascii_lowercase() {
            '0' => color = egui::Color32::from_rgb(0, 0, 0),
            '1' => color = egui::Color32::from_rgb(0, 0, 170),
            '2' => color = egui::Color32::from_rgb(0, 170, 0),
            '3' => color = egui::Color32::from_rgb(0, 170, 170),
            '4' => color = egui::Color32::from_rgb(170, 0, 0),
            '5' => color = egui::Color32::from_rgb(170, 0, 170),
            '6' => color = egui::Color32::from_rgb(255, 170, 0),
            '7' => color = egui::Color32::from_rgb(170, 170, 170),
            '8' => color = egui::Color32::from_rgb(85, 85, 85),
            '9' => color = egui::Color32::from_rgb(85, 85, 255),
            'a' => color = egui::Color32::from_rgb(85, 255, 85),
            'b' => color = egui::Color32::from_rgb(85, 255, 255),
            'c' => color = egui::Color32::from_rgb(255, 85, 85),
            'd' => color = egui::Color32::from_rgb(255, 85, 255),
            'e' => color = egui::Color32::from_rgb(255, 255, 85),
            'f' => color = egui::Color32::from_rgb(255, 255, 255),
            'k' => {}
            'l' => bold = true,
            'm' => strikethrough = true,
            'n' => underlined = true,
            'o' => italic = true,
            'r' => {
                color = egui::Color32::from_rgb(230, 230, 230);
                bold = false;
                italic = false;
                underlined = false;
                strikethrough = false;
            }
            _ => {}
        }
    }

    if !buf.is_empty() || out.is_empty() {
        out.push(ChatSegment {
            text: buf,
            color,
            bold,
            italic,
            underlined,
            strikethrough,
        });
    }
    out
}
