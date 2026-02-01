//! Automatic mod categorization based on name and content analysis

use super::InstalledMod;
use crate::db::Database;
use anyhow::Result;

/// Categorization rules based on mod name keywords
struct CategoryRule {
    category_name: &'static str,
    keywords: &'static [&'static str],
    file_patterns: &'static [&'static str],
}

const CATEGORY_RULES: &[CategoryRule] = &[
    // Bug fixes and patches (Main category)
    CategoryRule {
        category_name: "Bug Fixes",
        keywords: &[
            "bug", "fix", "patch", "hotfix", "unofficial", "ussep", "usleep", "uskp",
            "bugfix", "fixes", "repair", "correct", "correction", "enb fix", "enb helper",
            "skyrim se fixes", "se fixes", "engine fixes", "crash fix", "stability",
        ],
        file_patterns: &[],
    },

    // Structure/UI - Overhauls
    CategoryRule {
        category_name: "Overhauls",
        keywords: &[
            "campfire", "frostfall", "wet and cold", "realistic needs", "ineed",
            "hunterborn", "tentapalooza", "campfire tent", "survival", "hypothermia",
            "ordinator", "apocalypse", "vokrii", "adamant", "simon magus",
            "enai", "perkus maximus", "perma", "skyrim redone", "skyre",
            "requiem", "wildcat", "smilodon", "combat evolved", "vigor",
        ],
        file_patterns: &[],
    },

    // Structure/UI - Mission and Content Correction
    CategoryRule {
        category_name: "Mission and Content Correction",
        keywords: &[
            "cutting room floor", "crf", "open cities", "run for your lives",
            "when vampires attack", "timing is everything", "the choice is yours",
            "even better quest objectives", "ebqo", "quest expansion",
        ],
        file_patterns: &[],
    },

    // Structure/UI - Difficulty/Level List Mods
    CategoryRule {
        category_name: "Difficulty/Level List Mods",
        keywords: &[
            "difficulty", "level", "leveling", "leveled list", "deleveled",
            "challenge", "hardcore", "easier", "harder", "scaling",
            "morrowloot", "loot and degradation", "skyrim unleveled",
        ],
        file_patterns: &[],
    },

    // Structure/UI - Race Mods
    CategoryRule {
        category_name: "Race Mods",
        keywords: &[
            "race", "races", "playable race", "racial", "imperious",
            "khajiit", "argonian", "elf", "elven", "nord", "breton", "redguard",
            "orc", "orsimer", "vampire lord", "werewolf", "beast race",
        ],
        file_patterns: &[],
    },

    // Structure/UI - Perk Mods
    CategoryRule {
        category_name: "Perk Mods",
        keywords: &[
            "perk", "perks", "skill tree", "ordinator", "vokrii", "adamant",
            "perkus maximus", "perma", "skill", "abilities", "passive",
        ],
        file_patterns: &[],
    },

    // Structure/UI - UI Mods
    CategoryRule {
        category_name: "UI Mods",
        keywords: &[
            "skyui", "ui", "hud", "menu", "interface", "font", "mcm",
            "widget", "icon", "cursor", "notification", "message", "inventory",
            "better dialogue controls", "better message box", "immersive hud", "ihud",
            "a matter of time", "amot", "widget mod", "quick loot", "quick menus",
        ],
        file_patterns: &["interface/", "skse/plugins/skyui"],
    },

    // Structure/UI - Cheat Mods
    CategoryRule {
        category_name: "Cheat Mods",
        keywords: &[
            "cheat", "god mode", "ring of", "amulet of", "room", "console",
            "command", "infinite", "unlimited", "overpowered", "op ", "developer",
            "debug", "test", "spawn", "instant", "free", "cheat room",
        ],
        file_patterns: &[],
    },

    // Structure/UI - Frameworks (general)
    CategoryRule {
        category_name: "Structure and UI Mods",
        keywords: &[
            "skse", "framework", "engine", "core", "base", "requirement",
            "library", "resource", "platform", "foundation", "dll", "script extender",
            "papyrus", "skyrim script extender", "address library", "powerofthree",
            "po3", "jcontainers", "skyui", "mod configuration menu",
        ],
        file_patterns: &["skse/plugins/", "data/scripts/source"],
    },

    // Missions/Quests
    CategoryRule {
        category_name: "Missions/Quests",
        keywords: &[
            "quest", "mission", "story", "adventure", "dungeon", "cave",
            "realm", "island", "land", "expansion", "dlc", "journey",
            "tale", "saga", "campaign", "chapter", "beyond skyrim", "bs:",
            "the notice board", "missives", "headhunter", "moon and star",
            "falskaar", "wyrmstooth", "clockwork", "forgotten city",
            "vigilant", "glenmoril", "unslaad", "midwood isle",
        ],
        file_patterns: &[],
    },

    // Environmental - Global Mesh
    CategoryRule {
        category_name: "Global Mesh Mods",
        keywords: &[
            "smim", "static mesh", "high poly", "3d ", "noble skyrim",
            "skyland", "majestic mountains", "blended roads", "better dynamic",
            "improved closefaced helmets", "mesh improvement", "model replacement",
        ],
        file_patterns: &["meshes/clutter/", "meshes/furniture/"],
    },

    // Environmental - Weather/Lighting
    CategoryRule {
        category_name: "Weather/Lighting",
        keywords: &[
            "weather", "enb", "lighting", "light", "lux", "elfx", "climates",
            "lamp", "torch", "candle", "fire", "luminosity", "brightness",
            "shadow", "sunrise", "sunset", "storm", "rain", "fog",
            "cathedral weathers", "obsidian weathers", "vivid weathers", "rustic weathers",
            "enhanced lights", "relighting skyrim", "window shadows",
        ],
        file_patterns: &[],
    },

    // Environmental - Foliage
    CategoryRule {
        category_name: "Foliage Mods",
        keywords: &[
            "landscape", "terrain", "tree", "flora", "grass", "plant", "flower",
            "mountain", "snow", "water", "environment", "forest", "foliage",
            "leaf", "branch", "shrub", "bush", "vine", "moss", "rock", "stone",
            "river", "lake", "ocean", "sea", "beach", "aspens", "pines",
            "verdant", "skyrim flora overhaul", "sfo", "cathedral landscapes",
            "folkvangr", "origins of forest",
        ],
        file_patterns: &["meshes/landscape/", "textures/landscape/", "meshes/plants/"],
    },

    // Environmental - Sound
    CategoryRule {
        category_name: "Sound Mods",
        keywords: &[
            "sound", "audio", "music", "voice", "sfx", "ambient",
            "footstep", "effect", "acoustic", "soundtrack", "immersive sounds",
            "sounds of skyrim", "audio overhaul", "reverb", "personalized music",
            "musical lore", "celtic music", "the northerner diaries",
        ],
        file_patterns: &["sound/", "music/"],
    },

    // Buildings - Distributed Content
    CategoryRule {
        category_name: "Distributed Content",
        keywords: &[
            "dolmen", "oblivion gates", "ruins", "distributed", "worldwide",
            "scattered", "placed", "across skyrim", "throughout",
        ],
        file_patterns: &[],
    },

    // Buildings - Settlements
    CategoryRule {
        category_name: "Settlements",
        keywords: &[
            "city", "town", "village", "settlement", "holds", "whiterun",
            "solitude", "windhelm", "riften", "markarth", "winterhold",
            "morthal", "dawnstar", "falkreath", "riverwood", "rorikstead",
            "ivarstead", "shor's stone", "karthwasten", "expanded towns",
            "great cities", "jk's skyrim", "dawn of skyrim", "cities of the north",
        ],
        file_patterns: &["meshes/architecture/whiterun", "meshes/architecture/solitude"],
    },

    // Buildings - Individual Buildings
    CategoryRule {
        category_name: "Individual Buildings",
        keywords: &[
            "building", "house", "home", "player home", "hearthfire",
            "castle", "fort", "fortress", "stronghold", "tower", "hall",
            "manor", "estate", "keep", "hideout", "base",
            "elysium estate", "lakeview", "windstad", "heljarchen",
        ],
        file_patterns: &["meshes/architecture/"],
    },

    // Buildings - Building Interiors
    CategoryRule {
        category_name: "Building Interiors",
        keywords: &[
            "interior", "inside", "indoor", "clutter", "furniture",
            "décor", "decoration", "retexture", "interior lighting",
        ],
        file_patterns: &[],
    },

    // Items - Item Packs
    CategoryRule {
        category_name: "Item Packs",
        keywords: &[
            "pack", "collection", "bundle", "set of", "compilation",
            "immersive weapons", "immersive armors", "common clothes",
            "warmonger armory", "weapons of the third era",
        ],
        file_patterns: &[],
    },

    // Items - Individual Items (Weapons)
    CategoryRule {
        category_name: "Individual Items",
        keywords: &[
            "weapon", "sword", "axe", "mace", "dagger", "bow", "crossbow",
            "arrow", "bolt", "staff", "warhammer", "greatsword", "katana",
            "blade", "spear", "halberd", "scythe", "rapier", "saber",
            "claymore", "longsword", "shortsword", "scimitar",
        ],
        file_patterns: &["meshes/weapons/"],
    },

    // Items - Individual Items (Armor)
    CategoryRule {
        category_name: "Individual Items",
        keywords: &[
            "armor", "armour", "shield", "helmet", "gauntlet", "boot", "boots",
            "cuirass", "plate", "leather", "steel", "iron", "ebony",
            "daedric", "dragonbone", "dragonscale", "glass", "elven",
            "orcish", "dwarven", "chainmail", "brigandine",
        ],
        file_patterns: &["meshes/armor/"],
    },

    // Items - Individual Items (Clothing)
    CategoryRule {
        category_name: "Individual Items",
        keywords: &[
            "clothing", "cloth", "outfit", "robe", "dress", "shirt",
            "pants", "skirt", "coat", "cloak", "cape", "hood", "cowl",
            "glove", "shoe", "sandal", "belt", "jewelry", "ring",
            "necklace", "amulet", "circlet", "tiara", "crown",
        ],
        file_patterns: &["meshes/clothes/"],
    },

    // Items - General
    CategoryRule {
        category_name: "Items",
        keywords: &[
            "equipment", "item", "gear", "inventory", "container",
            "bag", "backpack", "pouch", "chest", "satchel",
            "bandolier", "warpaints", "warpaint",
        ],
        file_patterns: &[],
    },

    // Gameplay - AI Mods
    CategoryRule {
        category_name: "AI Mods",
        keywords: &[
            "immersive citizens", "ai overhaul", "realistic ai", "smarter",
            "intelligent", "behavior", "behaviour", "npc ai", "ai package",
        ],
        file_patterns: &[],
    },

    // Gameplay - Robust Gameplay
    CategoryRule {
        category_name: "Robust Gameplay Changes",
        keywords: &[
            "marriage all", "alternate start", "live another life", "lal",
            "realm of lorkhan", "skyrim unbound", "random alternate start",
            "arthmoor", "notice board", "missives",
        ],
        file_patterns: &[],
    },

    // Gameplay - Expanded Armor
    CategoryRule {
        category_name: "Expanded Armor",
        keywords: &[
            "magic books", "pouches", "cloaks of skyrim", "winter is coming",
            "bandolier", "bags and pouches", "wearable lanterns",
        ],
        file_patterns: &[],
    },

    // Gameplay - Crafting
    CategoryRule {
        category_name: "Crafting Mods",
        keywords: &[
            "craft", "crafting", "smith", "smithing", "forge", "alchemy",
            "enchanting", "cooking", "recipe", "complete alchemy",
            "complete crafting", "honed metal", "ars metallica",
        ],
        file_patterns: &[],
    },

    // Gameplay - Combat
    CategoryRule {
        category_name: "Other Gameplay",
        keywords: &[
            "combat", "fight", "battle", "attack", "defense", "parry",
            "dodge", "block", "stamina", "fatigue", "wound", "injury",
            "wildcat", "smilodon", "ultimate combat", "tk dodge",
            "mortal enemies", "athleticskillmod", "the ultimate dodge mod",
        ],
        file_patterns: &[],
    },

    // Gameplay - Magic
    CategoryRule {
        category_name: "Other Gameplay",
        keywords: &[
            "magic", "spell", "enchant", "conjur", "destruct", "restor",
            "illusion", "alteration", "mysticism", "mage", "wizard",
            "sorcerer", "magicka", "ritual", "rune", "apocalypse",
            "forgotten magic", "phenderix", "arcanum", "mysticism",
        ],
        file_patterns: &[],
    },

    // Gameplay - General
    CategoryRule {
        category_name: "Other Gameplay",
        keywords: &[
            "gameplay", "mechanic", "perk", "skill", "level", "balance",
            "overhaul", "realism", "immersive", "survival", "needs",
            "economy", "trade", "loot", "craft", "recipe", "smith",
            "alchemy", "cook", "cooking", "hunting", "fishing",
            "rich merchants", "faster greatswords", "movement behavior",
        ],
        file_patterns: &["scripts/"],
    },

    // Gameplay - Animation
    CategoryRule {
        category_name: "Other Gameplay",
        keywords: &[
            "animation", "anim", "pose", "idle", "emote", "gesture",
            "movement", "walk", "run", "jump", "locomotion", "dar",
            "dynamic animation", "nemesis", "fnis", "xpmse",
        ],
        file_patterns: &["meshes/actors/character/animations/", "meshes/actors/character/behaviors/"],
    },

    // NPCs - Overhauls
    CategoryRule {
        category_name: "NPC Overhauls",
        keywords: &[
            "diverse dragons", "dragon overhaul", "deadly dragons",
            "splendor dragon variants", "bellyaches", "varied dragons",
        ],
        file_patterns: &[],
    },

    // NPCs - Populated Series
    CategoryRule {
        category_name: "Populated Series",
        keywords: &[
            "populated", "inconsequential npcs", "travelers of skyrim",
            "interesting npcs", "3dnpc",
        ],
        file_patterns: &[],
    },

    // NPCs - Additions
    CategoryRule {
        category_name: "Other NPC Additions",
        keywords: &[
            "npc", "follower", "companion", "character", "people", "person",
            "immersive citizens", "citizen", "ai", "vendor", "merchant",
            "guard", "bandit", "enemy", "ally", "spouse", "marriage",
            "creature", "monster", "beast", "animal", "dragon", "wolf",
            "bear", "spider", "troll", "giant", "draugr", "vampire",
            "werewolf", "horse", "dog", "cat", "bird", "deer",
        ],
        file_patterns: &["meshes/actors/", "textures/actors/"],
    },

    // Appearance - Hair
    CategoryRule {
        category_name: "Hairdo Mods",
        keywords: &[
            "hair", "hairdo", "hairstyle", "kshairdo", "ks hairdo",
            "apachii", "yundao", "salt and wind",
        ],
        file_patterns: &["meshes/actors/character/character assets/hair"],
    },

    // Appearance - Adorable Females
    CategoryRule {
        category_name: "Adorable Females",
        keywords: &[
            "adorable", "cute female", "pretty female", "beautiful female",
            "bijin", "pandorable", "kalilies", "the ordinary women",
            "seranaholic", "fresh faces", "inhabitants of skyrim",
        ],
        file_patterns: &[],
    },

    // Appearance - Face
    CategoryRule {
        category_name: "Face Mods",
        keywords: &[
            "face", "facial", "brow", "eyebrow", "beard", "mustache",
            "lip", "lips", "mouth", "nose", "jaw", "cheek",
            "high poly head", "expressive facegen", "covereyes", "coverwomen",
        ],
        file_patterns: &["textures/actors/character/female/facetint"],
    },

    // Appearance - Body
    CategoryRule {
        category_name: "Body Mesh Mods",
        keywords: &[
            "body", "cbbe", "unp", "bhunp", "3ba", "3bbb",
            "bodyslide", "seraphim", "beautiful mistresses", "dimon99", "maevan",
            "tempered skins", "fair skin", "skysight", "vitruvia",
        ],
        file_patterns: &["meshes/actors/character", "calientetools/", "shapedata/"],
    },

    // Appearance - Eyes
    CategoryRule {
        category_name: "Natural Eyes",
        keywords: &[
            "eyes", "eye", "natural eyes", "improved eyes", "eye normal map",
            "pupil", "iris", "eyeball",
        ],
        file_patterns: &["textures/actors/character/female/eyes", "textures/actors/character/male/eyes"],
    },

    // Appearance - General
    CategoryRule {
        category_name: "Other Appearance",
        keywords: &[
            "racemenu", "preset", "beauty", "character", "cosmetic",
            "makeup", "teeth", "tattoo", "scar", "complexion",
            "bodypaints", "overlays", "slider", "morph", "warpaint",
        ],
        file_patterns: &["textures/actors/character/female"],
    },

    // Textures
    CategoryRule {
        category_name: "Texture Mods",
        keywords: &[
            "texture", "retexture", "4k", "2k", "8k", "1k", "hd", "uhd",
            "visual", "graphics", "upscale", "resolution", "parallax",
            "diffuse", "normal", "specular", "subsurface", "noble skyrim",
            "skyland", "majestic mountains", "rustic", "skyrim 2020",
            "pfuscher", "gamwich", "kajuan",
        ],
        file_patterns: &["textures/"],
    },

    // Patches - Compatibility
    CategoryRule {
        category_name: "Compatibility Patches",
        keywords: &[
            "patch", "compatibility", "compat", "apocalypse-ordinator",
            "enai patch", "fixes for", "patch for",
        ],
        file_patterns: &[],
    },

    // Patches - Content Altering
    CategoryRule {
        category_name: "Content Patches",
        keywords: &[
            "nerf", "buff", "balance", "tweak", "adjustment",
            "disable", "remove", "lite version",
        ],
        file_patterns: &[],
    },

    // Patches - Performance
    CategoryRule {
        category_name: "Performance/Disable Patches",
        keywords: &[
            "performance", "fps", "optimization", "optimized", "lite",
            "low end", "potato", "remove grass", "insignificant object",
            "go away clouds",
        ],
        file_patterns: &[],
    },
];

/// Automatically categorize a mod based on its name and file structure
pub async fn auto_categorize_mod(db: &Database, mod_info: &InstalledMod) -> Result<()> {
    // Convert mod name to lowercase for matching
    let mod_name_lower = mod_info.name.to_lowercase();

    // Get mod file paths if available
    let mod_files = db.get_mod_files(mod_info.id)?;
    let file_paths: Vec<String> = mod_files
        .iter()
        .map(|f| f.relative_path.to_lowercase())
        .collect();

    // Score each category rule
    let mut best_match: Option<(&str, u32)> = None;

    for rule in CATEGORY_RULES {
        let mut score = 0u32;

        // Check name keywords (weight: 10 per match)
        for keyword in rule.keywords {
            if mod_name_lower.contains(keyword) {
                score += 10;
            }
        }

        // Check file patterns (weight: 5 per match)
        for pattern in rule.file_patterns {
            if file_paths.iter().any(|path: &String| path.contains(pattern)) {
                score += 5;
            }
        }

        // Update best match if this score is higher
        if score > 0 {
            if let Some((_, best_score)) = best_match {
                if score > best_score {
                    best_match = Some((rule.category_name, score));
                }
            } else {
                best_match = Some((rule.category_name, score));
            }
        }
    }

    // If no match found, try fallback categorization based on file types
    let (final_category, score) = if let Some((cat, score)) = best_match {
        (cat, score)
    } else {
        // Fallback: categorize by predominant file type
        let has_textures = file_paths.iter().any(|p: &String| p.contains("textures/"));
        let has_meshes = file_paths.iter().any(|p: &String| p.contains("meshes/"));
        let has_scripts = file_paths.iter().any(|p: &String| p.contains("scripts/"));
        let has_sounds = file_paths.iter().any(|p: &String| p.contains("sound/") || p.contains("music/"));
        let has_interface = file_paths.iter().any(|p: &String| p.contains("interface/"));
        let has_esp = file_paths.iter().any(|p: &String| p.ends_with(".esp") || p.ends_with(".esm") || p.ends_with(".esl"));

        if has_textures && !has_meshes {
            ("Texture Mods", 3)
        } else if has_meshes && has_textures {
            // Has both meshes and textures - likely appearance or items
            if file_paths.iter().any(|p: &String| p.contains("actors/")) {
                ("Other Appearance", 3)
            } else if file_paths.iter().any(|p: &String| p.contains("weapons/") || p.contains("armor/")) {
                ("Individual Items", 3)
            } else if file_paths.iter().any(|p: &String| p.contains("architecture/") || p.contains("clutter/")) {
                ("Individual Buildings", 3)
            } else {
                ("Foliage Mods", 3)
            }
        } else if has_meshes && !has_textures {
            // Meshes only - check what type
            if file_paths.iter().any(|p: &String| p.contains("actors/")) {
                ("Other Appearance", 3)
            } else if file_paths.iter().any(|p: &String| p.contains("weapons/") || p.contains("armor/")) {
                ("Individual Items", 3)
            } else if file_paths.iter().any(|p: &String| p.contains("architecture/")) {
                ("Individual Buildings", 3)
            } else {
                ("Foliage Mods", 3)
            }
        } else if has_scripts {
            ("Other Gameplay", 3)
        } else if has_sounds {
            ("Sound Mods", 3)
        } else if has_interface {
            ("UI Mods", 3)
        } else if has_esp {
            // Has plugin files but no other clear indicators - likely gameplay
            ("Other Gameplay", 2)
        } else {
            // Unknown - leave uncategorized for manual assignment
            ("Patches", 1)
        }
    };

    // Assign category
    if let Some(category) = db.get_category_by_name(final_category)? {
        if let Some(category_id) = category.id {
            db.update_mod_category(mod_info.id, Some(category_id))?;
            tracing::info!(
                "Auto-categorized '{}' as '{}' (score: {})",
                mod_info.name,
                final_category,
                score
            );
        }
    }

    Ok(())
}

/// Automatically categorize all uncategorized mods for a game
pub async fn auto_categorize_all_mods(db: &Database, game_id: &str) -> Result<usize> {
    let mods = db.get_mods_for_game(game_id)?;
    let mut categorized = 0;

    for mod_record in mods {
        // Only auto-categorize mods that don't have a category yet
        if mod_record.category_id.is_none() {
            let installed_mod: InstalledMod = mod_record.into();
            auto_categorize_mod(db, &installed_mod).await?;
            categorized += 1;
        }
    }

    Ok(categorized)
}
