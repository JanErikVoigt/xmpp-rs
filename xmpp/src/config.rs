// Copyright (c) 2025 Crate authors
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/// Store Agent configuration. Differs from state which is generated at runtime
pub struct Config {
    /// Synchronize bookmarks based on autojoin flag.
    /// The client will join and leave based on the value of the `autojoin` flag on the (pubsub)
    /// bookmark item.
    /// If this `bookmarks_autojoin` attribute is set to false, `autojoin` set to false won't make
    /// the client leave a room, neither will the removal of a bookmark item. This will only happen
    /// after the client is restarted, as these items won't be automatically joined anymore.
    /// <https://xmpp.org/extensions/xep-0402.html#notification>
    pub bookmarks_autojoin: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    fn new() -> Self {
        Config {
            bookmarks_autojoin: true,
        }
    }
}
