//! Searchable emoji picker data.
//!
//! A curated set of common emojis with names, keywords and categories.
//! Actual glyph rendering depends on the system emoji font (e.g. Noto Color
//! Emoji on Linux); where no emoji font exists the OS falls back to monochrome
//! or placeholder glyphs — a platform limitation, not a data issue.

pub struct Emoji {
    pub char: &'static str,
    pub name: &'static str,
    pub keywords: &'static str,
    pub category: &'static str,
}

pub const EMOJIS: &[Emoji] = &[
    Emoji { char: "😀", name: "grinning face", keywords: "smile happy grin", category: "Smileys" },
    Emoji { char: "😂", name: "face with tears of joy", keywords: "laugh lol crying funny", category: "Smileys" },
    Emoji { char: "🥲", name: "smiling face with tear", keywords: "cry smile touched", category: "Smileys" },
    Emoji { char: "😍", name: "smiling face with heart-eyes", keywords: "love crush heart eyes", category: "Smileys" },
    Emoji { char: "🤔", name: "thinking face", keywords: "think hmm consider", category: "Smileys" },
    Emoji { char: "😎", name: "smiling face with sunglasses", keywords: "cool shades", category: "Smileys" },
    Emoji { char: "🥳", name: "partying face", keywords: "celebrate birthday party", category: "Smileys" },
    Emoji { char: "😴", name: "sleeping face", keywords: "sleep tired snore", category: "Smileys" },
    Emoji { char: "🤯", name: "exploding head", keywords: "mind blown shock wow", category: "Smileys" },
    Emoji { char: "🥺", name: "pleading face", keywords: "puppy eyes beg please", category: "Smileys" },
    Emoji { char: "😭", name: "loudly crying face", keywords: "cry sob tears sad", category: "Smileys" },
    Emoji { char: "😤", name: "face with steam from nose", keywords: "triumph angry huff", category: "Smileys" },
    Emoji { char: "🙂", name: "slightly smiling face", keywords: "smile ok", category: "Smileys" },
    Emoji { char: "🙃", name: "upside-down face", keywords: "silly sarcasm", category: "Smileys" },
    Emoji { char: "😉", name: "winking face", keywords: "wink flirt", category: "Smileys" },
    Emoji { char: "🤝", name: "handshake", keywords: "deal agree shake hands", category: "People" },
    Emoji { char: "👍", name: "thumbs up", keywords: "like approve yes good", category: "People" },
    Emoji { char: "👎", name: "thumbs down", keywords: "dislike no bad", category: "People" },
    Emoji { char: "👏", name: "clapping hands", keywords: "applause bravo praise", category: "People" },
    Emoji { char: "🙏", name: "folded hands", keywords: "pray please thanks", category: "People" },
    Emoji { char: "💪", name: "flexed biceps", keywords: "strong muscle gym", category: "People" },
    Emoji { char: "👋", name: "waving hand", keywords: "hello hi bye wave", category: "People" },
    Emoji { char: "✌️", name: "victory hand", keywords: "peace v sign", category: "People" },
    Emoji { char: "🤞", name: "crossed fingers", keywords: "luck hope wish", category: "People" },
    Emoji { char: "👀", name: "eyes", keywords: "look see watch", category: "People" },
    Emoji { char: "🧠", name: "brain", keywords: "smart think mind", category: "People" },
    Emoji { char: "❤️", name: "red heart", keywords: "love heart", category: "Symbols" },
    Emoji { char: "🧡", name: "orange heart", keywords: "love heart", category: "Symbols" },
    Emoji { char: "💔", name: "broken heart", keywords: "heartbreak sad", category: "Symbols" },
    Emoji { char: "🔥", name: "fire", keywords: "hot lit flame", category: "Nature" },
    Emoji { char: "⭐", name: "star", keywords: "favorite rate", category: "Nature" },
    Emoji { char: "🌙", name: "crescent moon", keywords: "night sleep", category: "Nature" },
    Emoji { char: "☀️", name: "sun", keywords: "sunny day weather", category: "Nature" },
    Emoji { char: "🌧️", name: "cloud with rain", keywords: "rain weather", category: "Nature" },
    Emoji { char: "🌈", name: "rainbow", keywords: "pride colors", category: "Nature" },
    Emoji { char: "🐛", name: "bug", keywords: "insect debug error", category: "Nature" },
    Emoji { char: "🐢", name: "turtle", keywords: "slow", category: "Nature" },
    Emoji { char: "🐧", name: "penguin", keywords: "linux tux", category: "Nature" },
    Emoji { char: "🦀", name: "crab", keywords: "rust ferris", category: "Nature" },
    Emoji { char: "🍕", name: "pizza", keywords: "food slice cheese", category: "Food" },
    Emoji { char: "☕", name: "hot beverage", keywords: "coffee tea drink", category: "Food" },
    Emoji { char: "🍺", name: "beer mug", keywords: "drink cheers", category: "Food" },
    Emoji { char: "🎉", name: "party popper", keywords: "celebrate congrats tada", category: "Objects" },
    Emoji { char: "🎁", name: "wrapped gift", keywords: "present birthday", category: "Objects" },
    Emoji { char: "💡", name: "light bulb", keywords: "idea bright", category: "Objects" },
    Emoji { char: "🔒", name: "locked", keywords: "lock secure private password", category: "Objects" },
    Emoji { char: "🔑", name: "key", keywords: "password unlock", category: "Objects" },
    Emoji { char: "📎", name: "paperclip", keywords: "attach clip", category: "Objects" },
    Emoji { char: "✂️", name: "scissors", keywords: "cut", category: "Objects" },
    Emoji { char: "📌", name: "pushpin", keywords: "pin important", category: "Objects" },
    Emoji { char: "📋", name: "clipboard", keywords: "copy paste", category: "Objects" },
    Emoji { char: "💾", name: "floppy disk", keywords: "save", category: "Objects" },
    Emoji { char: "💻", name: "laptop", keywords: "computer code dev", category: "Objects" },
    Emoji { char: "⌨️", name: "keyboard", keywords: "type keys", category: "Objects" },
    Emoji { char: "🖱️", name: "computer mouse", keywords: "click", category: "Objects" },
    Emoji { char: "📱", name: "mobile phone", keywords: "iphone smartphone", category: "Objects" },
    Emoji { char: "🔋", name: "battery", keywords: "power charge", category: "Objects" },
    Emoji { char: "🔌", name: "electric plug", keywords: "power plug", category: "Objects" },
    Emoji { char: "🚀", name: "rocket", keywords: "launch ship fast", category: "Travel" },
    Emoji { char: "✈️", name: "airplane", keywords: "flight travel plane", category: "Travel" },
    Emoji { char: "🚗", name: "automobile", keywords: "car drive", category: "Travel" },
    Emoji { char: "🚲", name: "bicycle", keywords: "bike cycle", category: "Travel" },
    Emoji { char: "🏠", name: "house", keywords: "home", category: "Travel" },
    Emoji { char: "⚽", name: "soccer ball", keywords: "football sport", category: "Activity" },
    Emoji { char: "🎮", name: "video game", keywords: "gaming controller play", category: "Activity" },
    Emoji { char: "🎧", name: "headphone", keywords: "music audio", category: "Activity" },
    Emoji { char: "🎵", name: "musical note", keywords: "music song", category: "Activity" },
    Emoji { char: "📚", name: "books", keywords: "read study", category: "Activity" },
    Emoji { char: "✅", name: "check mark button", keywords: "done yes check ok", category: "Symbols" },
    Emoji { char: "❌", name: "cross mark", keywords: "no x delete wrong", category: "Symbols" },
    Emoji { char: "⚠️", name: "warning", keywords: "alert caution", category: "Symbols" },
    Emoji { char: "❓", name: "red question mark", keywords: "question help", category: "Symbols" },
    Emoji { char: "💯", name: "hundred points", keywords: "100 perfect score", category: "Symbols" },
    Emoji { char: "➕", name: "plus", keywords: "add more", category: "Symbols" },
    Emoji { char: "➖", name: "minus", keywords: "subtract less", category: "Symbols" },
    Emoji { char: "©️", name: "copyright", keywords: "c rights", category: "Symbols" },
    Emoji { char: "👍🏽", name: "thumbs up medium skin tone", keywords: "like approve yes", category: "People" },
    Emoji { char: "👋🏽", name: "waving hand medium skin tone", keywords: "hello hi bye", category: "People" },
    Emoji { char: "🙏🏽", name: "folded hands medium skin tone", keywords: "pray thanks", category: "People" },
    Emoji { char: "👏🏽", name: "clapping hands medium skin tone", keywords: "applause bravo", category: "People" },
    Emoji { char: "💪🏽", name: "flexed biceps medium skin tone", keywords: "strong muscle", category: "People" },
];

/// Search by name, keyword, or category. A leading `:` is treated as an
/// explicit emoji trigger and stripped.
pub fn search_emoji(query: &str) -> Vec<&'static Emoji> {
    let q = query.trim().strip_prefix(':').unwrap_or(query.trim());
    if q.len() < 2 {
        return Vec::new();
    }
    let q = q.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    EMOJIS
        .iter()
        .filter(|e| {
            let hay = format!("{} {} {} {}", e.name, e.keywords, e.category, e.char);
            words.iter().all(|w| hay.contains(w))
        })
        .take(6)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::search_emoji;

    #[test]
    fn finds_by_keyword() {
        let hits = search_emoji("laugh");
        assert!(hits.iter().any(|e| e.char == "😂"));
        let hits = search_emoji(":coffee");
        assert!(hits.iter().any(|e| e.char == "☕"));
        assert!(search_emoji("x").is_empty());
    }
}
