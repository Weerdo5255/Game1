// QUIETLY, EVERYTHING: miniquad plugin for the wasm build. It keeps the single save slot in
// localStorage and hands the URL query string to the flag parser. Load it after mq_js_bundle.js
// and before load(...). The Rust side is crates/game/src/storage.rs.
"use strict";
(function () {
    var KEY = "quietly_everything_save";
    var encoder = new TextEncoder();
    var decoder = new TextDecoder();
    // Bytes measured by a *_len call, waiting for the matching *_into call to copy them out.
    var pendingSave = null;
    var pendingQuery = null;

    function savedBytes() {
        try {
            var text = window.localStorage.getItem(KEY);
            return text === null ? null : encoder.encode(text);
        } catch (e) {
            return null;
        }
    }

    // "?autoplay&seed=3" becomes "autoplay&seed=3", each part URL-decoded.
    function queryBytes() {
        var parts = window.location.search.replace(/^\?/, "").split("&").filter(function (p) { return p.length > 0; });
        var decoded = parts.map(function (p) {
            try {
                return decodeURIComponent(p.replace(/\+/g, " "));
            } catch (e) {
                return p;
            }
        });
        return encoder.encode(decoded.join("&"));
    }

    function copyOut(bytes, ptr) {
        if (bytes !== null && bytes.length > 0) {
            new Uint8Array(wasm_memory.buffer, ptr, bytes.length).set(bytes);
        }
    }

    miniquad_add_plugin({
        register_plugin: function (importObject) {
            importObject.env.qe_save = function (ptr, len) {
                try {
                    window.localStorage.setItem(KEY, decoder.decode(new Uint8Array(wasm_memory.buffer, ptr, len)));
                } catch (e) {
                    console.warn("qe_save failed", e);
                }
            };
            // -1 when there is no save; otherwise the byte length Rust must allocate.
            importObject.env.qe_load_len = function () {
                pendingSave = savedBytes();
                return pendingSave === null ? -1 : pendingSave.length;
            };
            importObject.env.qe_load_into = function (ptr) {
                copyOut(pendingSave, ptr);
                pendingSave = null;
            };
            importObject.env.qe_query_len = function () {
                pendingQuery = queryBytes();
                return pendingQuery.length;
            };
            importObject.env.qe_query_into = function (ptr) {
                copyOut(pendingQuery, ptr);
                pendingQuery = null;
            };
        },
        on_init: function () {},
        name: "qe",
        version: 1
    });
})();
