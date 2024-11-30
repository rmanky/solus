use rand::seq::SliceRandom;
use twilight_model::gateway::presence::{ Activity, ActivityType, MinimalActivity };

struct ActivityData<'a> {
    name: &'a str,
    kind: ActivityType,
}

static ACTIVITIES: &[ActivityData] = &[
    ActivityData {
        name: "the sky fall",
        kind: ActivityType::Watching,
    },
    ActivityData {
        name: "lofi hip-hop 🎶",
        kind: ActivityType::Listening,
    },
    ActivityData {
        name: "with your heart ❤️",
        kind: ActivityType::Playing,
    },
    ActivityData {
        name: "and learning 📝",
        kind: ActivityType::Watching,
    },
];

pub fn get_random_activity() -> Activity {
    let activity = ACTIVITIES.choose(&mut rand::thread_rng()).unwrap();

    let minimal_activity = MinimalActivity {
        name: activity.name.to_string(),
        kind: activity.kind,
        url: None,
    };
    Activity::from(minimal_activity)
}
