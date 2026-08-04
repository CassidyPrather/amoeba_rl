// Opening a link from inside the canvas.
//
// This one is ours, not vendored — CREDITS.md tracks the files that are not.
// `src/links.rs` calls amoeba_open_link with a position in its `LINKS` array,
// and the array below is that same order. The two lists have to be edited
// together; nothing checks them against each other at build time.
"use strict";

const AMOEBA_LINKS = [
    "https://wirenook.net/",
    "https://github.com/CassidyPrather/amoeba_rl",
];

function amoeba_links_register_plugin(importObject) {
    importObject.env.amoeba_open_link = function (which) {
        const url = AMOEBA_LINKS[which];
        if (url === undefined) {
            return;
        }
        // The press that asked for this landed a frame ago at most, so the
        // browser's transient activation is still live and a new tab is
        // allowed. If one is refused anyway, going there in this tab beats
        // appearing to do nothing — the game is a page reload away.
        //
        // Not the "noopener" feature string: passing it makes window.open
        // return null whether or not the tab opened, which reads here as a
        // refusal and navigates this tab as well. Clearing the new tab's
        // opener afterwards is the same protection and still answers.
        const tab = window.open(url, "_blank");
        if (tab) {
            tab.opener = null;
        } else {
            window.location.href = url;
        }
    };
}

// The version pair is what stops gl.js warning that this plugin looks like an
// old one. It still logs that the Rust side exports no matching version
// function, which is true: exporting one would cost more unsafe than the whole
// link is worth.
miniquad_add_plugin({
    register_plugin: amoeba_links_register_plugin,
    name: "amoeba_links",
    version: 1,
});
