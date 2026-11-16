use std::collections::{BTreeMap, BTreeSet};

use app_core::clock::{Clock, Zone};
use app_core::platform::{Env, Os};
use app_core::service::{self, App};
use app_core::testkit::{TempDir, fixed_time};

use crate::model::Options;
use crate::render::Picture;
use crate::shots;

pub(crate) fn environment(home: &TempDir) -> Env {
    let directory = home.path().to_string_lossy().into_owned();
    Env::from_pairs(&[("HOME", directory.as_str())])
}

pub(crate) fn open_app(home: &TempDir) -> App {
    App::open(service::Options {
        os: Os::Linux,
        env: environment(home),
        config_dir: None,
        database_path: None,
        overrides: BTreeMap::new(),
        clock: Clock::fixed(fixed_time()),
        zone: Zone::Utc,
        read_only: false,
    })
    .unwrap()
}

pub(crate) fn options(home: &TempDir) -> Options {
    Options {
        env: environment(home),
        system_dark: false,
        autostart: None,
        in_terminal: false,
        open_terminal: None,
        open_url: None,
    }
}

fn tints(picture: &Picture) -> usize {
    picture.pixels.as_chunks::<4>().0.iter().collect::<BTreeSet<_>>().len()
}

#[test]
fn every_main_screen_renders_to_a_picture() {
    let home = TempDir::new("gui-screens");
    let mut app = open_app(&home);
    let pictures = shots::screens(&mut app, options(&home));
    let names: Vec<&str> = pictures.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, shots::NAMES);
    for (name, picture) in &pictures {
        assert!(picture.width >= 400 && picture.height >= 200, "{name} is {}x{}", picture.width, picture.height);
        assert!(tints(picture) > 24, "{name} shows only {} colors", tints(picture));
    }
    let light = &pictures.iter().find(|(name, _)| *name == "settings").unwrap().1;
    let dark = &pictures.iter().find(|(name, _)| *name == "settings-dark").unwrap().1;
    let brightness = |picture: &Picture| {
        picture.pixels.as_chunks::<4>().0.iter().map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2])).sum::<u64>()
    };
    assert!(brightness(dark) * 2 < brightness(light));
}
