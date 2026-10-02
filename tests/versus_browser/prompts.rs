//! The benchmark's prompts, frozen before the counted runs (`vs1` and `vs2`
//! drew the flyer's photo; from `vs3` it is a real photo). Every arm gets the same system prompt and task body; only the
//! tools paragraph differs.

/// Prompt version; runs are compared only with runs on the same version.
pub const VERSION: &str = "vs3";

/// The system prompt, the same for every arm.
pub const SYSTEM: &str = "You make images. Work autonomously; don't ask questions.";

/// The three setups compared.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arm {
    /// The keyline MCP server alone.
    Keyline,
    /// Bash, Write, Edit, Read and Playwright's screenshot CLI.
    BrowserCli,
    /// Playwright MCP plus Write, Edit, Read.
    BrowserMcp,
}

impl Arm {
    pub fn parse(s: &str) -> Arm {
        match s {
            "keyline" => Arm::Keyline,
            "browser-cli" => Arm::BrowserCli,
            "browser-mcp" => Arm::BrowserMcp,
            _ => panic!("KEYLINE_BENCH_ARM must be keyline, browser-cli or browser-mcp"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Arm::Keyline => "keyline",
            Arm::BrowserCli => "browser-cli",
            Arm::BrowserMcp => "browser-mcp",
        }
    }
}

/// The two pre-registered tasks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Task {
    /// The vote-by-mail reference ad, from `llm_e2e` without keyline words.
    ReferenceAd,
    /// A conference speaker card, written before any run.
    SpeakerCard,
}

impl Task {
    pub fn parse(s: &str) -> Task {
        match s {
            "reference-ad" => Task::ReferenceAd,
            "speaker-card" => Task::SpeakerCard,
            _ => panic!("KEYLINE_BENCH_TASK must be reference-ad or speaker-card"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Task::ReferenceAd => "reference-ad",
            Task::SpeakerCard => "speaker-card",
        }
    }

    /// Size id, width, height and content scale (1 unless the task says).
    pub fn sizes(self) -> [(&'static str, u32, u32, f32); 3] {
        match self {
            Task::ReferenceAd => [
                ("portrait", 1080, 1350, 1.0),
                ("wide", 1200, 1000, 0.85),
                ("sky", 300, 600, 0.28),
            ],
            Task::SpeakerCard => [
                ("square", 1080, 1080, 1.0),
                ("landscape", 1920, 1080, 1.0),
                ("story", 1080, 1920, 1.0),
            ],
        }
    }

    /// Asset ids with the file name the browser arms get.
    pub fn assets(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Task::ReferenceAd => &[
                ("photo", "photo.jpg"),
                ("mail", "mail.svg"),
                ("check", "check.svg"),
            ],
            Task::SpeakerCard => &[("logo", "logo.svg"), ("portrait", "portrait.png")],
        }
    }

    /// The task text with the arm's tools paragraph filled in.
    pub fn prompt(self, tools: &str) -> String {
        match self {
            Task::ReferenceAd => REFERENCE_AD,
            Task::SpeakerCard => SPEAKER_CARD,
        }
        .replace("{tools}", tools)
    }

    /// The content list, as the judge gets it.
    pub fn content(self) -> &'static str {
        let body = match self {
            Task::ReferenceAd => REFERENCE_AD,
            Task::SpeakerCard => SPEAKER_CARD,
        };
        let start = body.find("Content").expect("content list");
        let end = body.find("\n\nEvery size").expect("closing clause");
        &body[start..end]
    }
}

/// The tools paragraph for `arm`. `place` is the scene id (keyline) or the
/// working folder (browser arms); `port` is the browser-mcp arm's server.
pub fn tools(arm: Arm, task: Task, place: &str, port: u16) -> String {
    let ids: Vec<&str> = task.assets().iter().map(|(id, _)| *id).collect();
    let files: Vec<&str> = task.assets().iter().map(|(_, f)| *f).collect();
    let files = format!("{} and Inter.ttf", files.join(", "));
    let pngs: Vec<String> = task
        .sizes()
        .iter()
        .map(|(id, ..)| format!("{id}.png"))
        .collect();
    let pngs = format!("{} and {}", pngs[..2].join(", "), pngs[2]);
    match arm {
        Arm::Keyline => format!(
            "Use the scene MCP tools. Scene {place} already has the three sizes and the assets \
(ids {}). Render when done.",
            ids.join(", ")
        ),
        Arm::BrowserCli => format!(
            "Work in the current folder ({place}): {files} are here (load the font with @font-face). \
Render each size with `playwright screenshot --viewport-size \"W,H\" file://<abs path> <size>.png`. \
Save {pngs} here, at exactly those pixel sizes."
        ),
        Arm::BrowserMcp => format!(
            "Work in the current folder ({place}): {files} are here (load the font with @font-face). \
The folder is served at http://localhost:{port}/. Render each size with the Playwright MCP \
browser tools and save {pngs} in this folder, at exactly those pixel sizes."
        ),
    }
}

const REFERENCE_AD: &str = "Build a vote-by-mail flyer at three sizes and produce one PNG per size.
Sizes: portrait 1080×1350 (master), wide 1200×1000 with content at 0.85× the master's scale, \
sky 300×600 with content at 0.28× scale.
Assets: photo (1600×1000 photo: a farmhouse in a field at sunrise), mail (white envelope icon, 56×44 SVG), \
check (red check-circle icon, 40×40 SVG). Font: Inter.

{tools}

Content, top to bottom:
1. Headline \"Proven RESULTS for WILLOWMERE Families\", navy #1B2A5C, with RESULTS and WILLOWMERE in red #D0202E.
2. A full-width photo band.
3. Three candidate columns, evenly spaced: Dana Levi (Mayor), Omar Haddad (Council), Ruth Cohen (Council).
4. A full-width red call-to-action bar with the mail icon and \"VOTE BY MAIL\" in white.
5. Three steps, each with the check icon: \"Request your ballot by October 20\", \"Fill it out at home\", \
\"Mail it back by November 3\".
6. Footer: \"Paid for by Willowmere Forward · willowmereforward.org\".

Every size must look right: bands stay full width, columns stay evenly spaced, the photo crops \
instead of distorting, and no text is cut off, overflowing or overlapping. Check your result and \
fix every defect before you finish.";

const SPEAKER_CARD: &str =
    "Build a conference speaker card at three sizes and produce one PNG per size.
Sizes: square 1080×1080 (master), landscape 1920×1080, story 1080×1920.
Assets: logo (event logo mark, violet and white, 160×48 SVG), portrait (800×800 photo of the \
speaker). Font: Inter.

{tools}

Content:
1. The event logo, with the event name \"FIELDNOTES CONF 2026\" beside it.
2. The speaker's portrait, cropped to a circle.
3. The speaker's name \"Maya Okonkwo-Lindqvist\" and role \"Principal Engineer, Northwind Labs\".
4. The talk title, large: \"Shipping at the speed of trust: what ten years of on-call taught us \
about resilient systems\".
5. Date and venue: \"November 14, 2026 · Harbor Hall, Lisbon\".
6. A pill-shaped \"Get tickets\" button.
Colors: background #0F1226, text white, accent violet #7C5CFF.

Every size must look right: the portrait stays a circle, the long title wraps instead of being \
cut off, and no text is cut off, overflowing or overlapping. Check your result and fix every \
defect before you finish.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_tools_paragraph_differs() {
        for task in [Task::ReferenceAd, Task::SpeakerCard] {
            let a = task.prompt("X");
            let b = task.prompt("Y");
            assert_eq!(a.replace('X', "Y"), b);
            assert!(task.content().starts_with("Content"));
            assert!(task.content().contains("6. "));
        }
        let t = tools(Arm::BrowserCli, Task::ReferenceAd, "/w", 0);
        assert!(t.contains("photo.jpg, mail.svg, check.svg and Inter.ttf"));
        assert!(t.contains("portrait.png, wide.png and sky.png"));
        let t = tools(Arm::Keyline, Task::SpeakerCard, "s1", 0);
        assert!(t.contains("(ids logo, portrait)"));
    }
}
