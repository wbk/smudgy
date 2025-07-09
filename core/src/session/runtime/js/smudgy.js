const {
    op_smudgy_get_current_session,
    op_smudgy_get_session_character,
    op_smudgy_get_sessions,
    op_smudgy_create_simple_alias,
    op_smudgy_create_javascript_function_alias,
    op_smudgy_create_simple_trigger,
    op_smudgy_create_javascript_function_trigger,
    op_smudgy_set_alias_enabled,
    op_smudgy_set_trigger_enabled,
    op_smudgy_session_echo,
    op_smudgy_session_reload,
    op_smudgy_session_send,
    op_smudgy_session_send_raw,
    op_smudgy_insert,
    op_smudgy_replace,
    op_smudgy_highlight,
    op_smudgy_remove,
    op_smudgy_gag,
    op_smudgy_get_current_line,
    op_smudgy_get_current_line_number,
    op_smudgy_line_insert,
    op_smudgy_line_replace,
    op_smudgy_line_highlight,
    op_smudgy_line_remove,

} = Deno.core.ops;


/**
 * @typedef {Object} Character
 * @property {string} [name]
 * @property {string} [subtext]
 */

/**
 * Represents a session in the Smudgy MUD client.
 *
 * A Session object provides methods to interact with a specific MUD session,
 * such as sending commands, echoing text, and reloading the session's scripts.
 *
 * @class
 */
class Session {
    constructor(id) {
        this._id = id;
    }

    /**
     * Gets the ID of the session.
     *
     * @returns {string} The ID of the session.
     */
    get id() {
        return this._id;
    }
    set id(value) {
        this._id = value;
    }

    /**
     * Echoes a line of text to the terminal
     *
     * @param {string} line - The text to echo.
     */
    echo(line) {
        op_smudgy_session_echo(this.id, line);
    }

    /**
     * Reloads the session's scripts.
     */
    reload() {
        op_smudgy_session_reload(this.id);
    }

    /**
     * Sends a line of text to the MUD server, which will be processed exactly as if it were typed by the user.
     *
     * @param {string} line - The text to send.
     */
    send(line) {
        op_smudgy_session_send(this.id, line);
    }

    /**
     * Sends a raw line of text to the MUD server without any processing.
     *
     * @param {string} line - The raw text to send.
     */
    sendRaw(line) {
        op_smudgy_session_send_raw(this.id, line);
    }

    /**
     * Gets the character associated with this session.
     *
     * @returns {Character} The character name.
     */
    get character() {
        return op_smudgy_get_session_character(this.id);
    }

    /**
     * Returns a string representation of the Session object.
     *
     * @returns {string} A string representation of the Session.
     */
    toString() {
        return `Session(${this.id})`;
    }
}

class Alias {
    constructor(name) {
        this.name = name;
    }

    set enabled(value) {
        op_smudgy_set_alias_enabled(this.name, value);
    }
}

class Trigger {
    constructor(name) {
        this.name = name;
    }

    set enabled(value) {
        op_smudgy_set_trigger_enabled(this.name, value);
    }
}

Object.defineProperty(globalThis, "currentSession", {
    get() {
        return new Session(op_smudgy_get_current_session());
    },
});

Object.defineProperty(globalThis, "sessions", {
    get() {
        return op_smudgy_get_sessions().map((id) => new Session(id));
    },
});

/**
 * Sends a line of text to the current session.
 * @param {string} line - The line of text to send
 */
const send = (line) => currentSession.send(line);

Object.defineProperty(globalThis, "send", {
    value: send
});

/**
 * Sends a line of text to the current session without any processing.
 * @param {string} line - The raw line of text to send
 */
const sendRaw = (line) => currentSession.sendRaw(line);

Object.defineProperty(globalThis, "sendRaw", {
    value: sendRaw
});

/**
 * Echoes a line of text to the current session's output.
 * @param {string} line - The line of text to echo
 */
const echo = (line) => currentSession.echo(line);

Object.defineProperty(globalThis, "echo", {
    value: echo
});


/**
 * Creates an alias that matches patterns and executes a script
 * @param {string} name - The name of the alias
 * @param {string|RegExp|Array<string|RegExp>} patterns - The pattern(s) to match
 * @param {string|Function} script - The script to execute when the alias matches
 * @returns {Alias} The created alias
 */
function createAlias(name, patterns, script) {
    if (typeof name !== "string" || !/^\w+$/.test(name)) {
        throw new TypeError(
            `Name must be a non-empty string using only alphanumeric characters and underscores.
            
            Usage: createAlias("myAlias", /^my pattern$/, "my script")
            
            You provided: "${name}"`,
        );
    }

    patterns = Array.isArray(patterns) ? patterns : [patterns];
    patterns = patterns.map((p) => p instanceof RegExp ? p.source : p);

    if (script instanceof Function) {
        op_smudgy_create_javascript_function_alias(name, patterns, script);
    } else {
        op_smudgy_create_simple_alias(name, patterns, script);
    }

    return new Alias(name);
}

/**
 * @typedef {Object} TriggerDef
 * @property {(string|RegExp)[]} [patterns] - The pattern(s) to match.
 * @property {(string|RegExp)[]} [rawPatterns] - The raw pattern(s) to match
 * @property {(string|RegExp)[]} [antiPatterns] - The anti-pattern(s) to exclude
 * @property {string|Function} script - The script to execute when the trigger matches
 * @property {boolean} [prompt] - Should this trigger fire when a prompt is received, in addition to firing when new lines are received
 * @property {boolean} [enabled] - Should this trigger be enabled by default
 */

/**
 * Creates multiple triggers from an object of trigger definitions
 * @param {Object.<string, TriggerDef>} triggers - The triggers to create
 * @returns {Object.<string, Trigger>} The created triggers
 */
function createTriggers(triggers) {
    return Object.fromEntries(
        Object.entries(triggers).map(([name, triggerDef]) => {
            const {
                script,
                patterns,
                rawPatterns,
                antiPatterns,
                prompt,
                enabled,
            } = triggerDef;

            const validPatterns = {
                ...(patterns && { patterns }),
                ...(rawPatterns && { rawPatterns }),
                ...(antiPatterns && { antiPatterns }),
            };

            const options = {
                ...(prompt !== undefined && { prompt }),
                ...(enabled !== undefined && { enabled }),
            };

            return [name, createTrigger(name, validPatterns, script, options)];
        }),
    );
}
/**
 * @typedef {Object} TriggerPatterns
 * @property {(string|RegExp)[]} [patterns] - The pattern(s) to match.
 * @property {(string|RegExp)[]} [rawPatterns] - The raw pattern(s) to match
 * @property {(string|RegExp)[]} [antiPatterns] - The anti-pattern(s) to exclude
 */

/**
 * @typedef {Object} TriggerOptions
 * @property {boolean} [prompt] - (Default: false) Should this trigger fire when a prompt is received, in addition to firing when new lines are received
 * @property {boolean} [enabled] - (Default: true) Should this trigger be enabled by default
 */

/**
 * Creates a new trigger
 * @param {string} name - The name of the trigger
 * @param {String|RegExp|TriggerPatterns} patterns - The pattern(s) which fire the trigger
 * @param {string|Function} script - The script to execute when the trigger matches
 * @param {TriggerOptions} [options] - The options for the trigger
 * @returns {Trigger} The created trigger
 * @throws {TypeError} If the input is invalid
 */
function createTrigger(name, patterns, script, options = {}) {
    const params = validateCreateTriggerParams(name, patterns, script, options);

    if (typeof script === "function") {
        op_smudgy_create_javascript_function_trigger(
            params.name,
            params.normalizedPatterns.patterns,
            params.normalizedPatterns.rawPatterns,
            params.normalizedPatterns.antiPatterns,
            params.script,
            options.prompt ?? false,
            options.enabled ?? true,
        );
    } else {
        op_smudgy_create_simple_trigger(
            params.name,
            params.normalizedPatterns.patterns,
            params.normalizedPatterns.rawPatterns,
            params.normalizedPatterns.antiPatterns,
            params.script,
            options.prompt ?? false,
            options.enabled ?? true,
        );
    }

    return new Trigger(name);
}

/**
 * Validates and normalizes parameters for creating a trigger
 * @private
 * @param {string} name - The name of the trigger
 * @param {String|RegExp|TriggerPatterns} patterns - The pattern(s) which fire the trigger
 * @param {string|Function} script - The script to execute when the trigger matches
 * @param {TriggerOptions} options - The options for the trigger
 * @returns {Object} The validated and normalized parameters
 * @throws {TypeError} If any of the input parameters are invalid
 */

function validateCreateTriggerParams(name, patterns, script, options) {
    if (typeof name !== "string" || !/^\w+$/.test(name)) {
        throw new TypeError(
            "Name must be a non-empty string using only alphanumeric characters and underscores",
        );
    }
    if (typeof options !== "object" || options === null) {
        throw new TypeError("Options must be an object");
    }

    if ("prompt" in options && typeof options.prompt !== "boolean") {
        throw new TypeError('Option "prompt" must be a boolean');
    }

    if ("enabled" in options && typeof options.enabled !== "boolean") {
        throw new TypeError('Option "enabled" must be a boolean');
    }

    // Check for unexpected options
    const validOptions = ["prompt", "enabled"];
    const unexpectedOptions = Object.keys(options).filter((key) =>
        !validOptions.includes(key)
    );
    if (unexpectedOptions.length > 0) {
        throw new TypeError(
            `Unexpected option(s): ${unexpectedOptions.join(", ")}`,
        );
    }

    if (
        typeof patterns !== "string" && !(patterns instanceof RegExp) &&
        typeof patterns !== "object"
    ) {
        throw new TypeError(
            "Patterns must be a string, RegExp, or an object with pattern properties",
        );
    }

    if (typeof script !== "string" && typeof script !== "function") {
        throw new TypeError("Script must be a string or function");
    }

    const normalizedPatterns = normalizePatterns(patterns);
    if (
        normalizedPatterns.patterns.length === 0 &&
        normalizedPatterns.rawPatterns.length === 0
    ) {
        throw new TypeError(
            "At least one pattern or raw pattern must be provided",
        );
    }

    return { name, normalizedPatterns, script };
}

/**
 * Normalizes the patterns for a trigger
 * @private
 * @param {String|RegExp|TriggerPatterns} patterns - The pattern(s) to normalize
 * @returns {Object} An object containing normalized patterns, raw patterns, and anti-patterns
 */
function normalizePatterns(patterns) {
    const normalized = {
        patterns: [],
        rawPatterns: [],
        antiPatterns: [],
    };

    if (typeof patterns === "string" || patterns instanceof RegExp) {
        normalized.patterns = [
            patterns instanceof RegExp ? patterns.source : patterns,
        ];
    } else if (typeof patterns === "object") {
        normalized.patterns = (patterns.patterns || []).map((p) =>
            p instanceof RegExp ? p.source : p
        );
        normalized.rawPatterns = (patterns.rawPatterns || []).map((p) =>
            p instanceof RegExp ? p.source : p
        );
        normalized.antiPatterns = (patterns.antiPatterns || []).map((p) =>
            p instanceof RegExp ? p.source : p
        );
    }

    return normalized;
}

/**
 * Global line manipulation object for modifying incoming lines
 * 
 * Line operations are queued and applied to the next incoming line from the server.
 * Multiple operations can be queued and will be applied in the order they were added.
 * 
 * @namespace line
 */
const line = {
    /**
     * Inserts text at the specified position with optional styling
     * @param {string} text - The text to insert
     * @param {number} begin - The start position to insert at
     * @param {number} [end] - The end position (for replacement), defaults to begin
     * @param {Object} [options] - Styling options
     * @param {string|Object} [options.fg] - Foreground color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     * @param {string|Object} [options.bg] - Background color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     */
    insert(text, begin, end = begin, options = {}) {
        op_smudgy_insert(text, begin, end, options.fg || null, options.bg || null);
    },

    /**
     * Replaces text in the specified range
     * @param {string} text - The replacement text
     * @param {number} begin - The start position to replace
     * @param {number} end - The end position to replace
     */
    replaceAt(text, begin, end) {
        op_smudgy_replace(text, begin, end);
    },

    /**
     * Highlights text in the specified range with the given colors
     * @param {number} begin - The start position to highlight
     * @param {number} end - The end position to highlight
     * @param {Object} [options] - Styling options
     * @param {string|Object} [options.fg] - Foreground color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     * @param {string|Object} [options.bg] - Background color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     */
    highlightAt(begin, end, options = {}) {
        op_smudgy_highlight(begin, end, options.fg || null, options.bg || null);
    },

    /**
     * Removes text in the specified range
     * @param {number} begin - The start position to remove
     * @param {number} end - The end position to remove
     */
    removeAt(begin, end) {
        op_smudgy_remove(begin, end);
    },

    /**
     * Replaces the first occurrence of oldStr with newStr in the current line
     * @param {string} oldStr - The text to find and replace
     * @param {string} newStr - The replacement text
     * @returns {boolean} True if the text was found and replaced, false otherwise
     */
    replace(oldStr, newStr) {
        const currentText = op_smudgy_get_current_line();
        const index = currentText.indexOf(oldStr);
        if (index !== -1) {
            this.replaceAt(newStr, index, index + oldStr.length);
        }
    },

    /**
     * Highlights the first occurrence of str in the current line
     * @param {string} str - The text to find and highlight
     * @param {Object} [options] - Styling options
     * @param {string|Object} [options.fg] - Foreground color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     * @param {string|Object} [options.bg] - Background color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     * @returns {boolean} True if the text was found and highlighted, false otherwise
     */
    highlight(str, options = {}) {
        const currentText = op_smudgy_get_current_line();
        const index = currentText.indexOf(str);
        if (index !== -1) {
            this.highlightAt(index, index + str.length, options);
        }
    },

    /**
     * Removes the first occurrence of str from the current line
     * @param {string} str - The text to find and remove
     * @returns {boolean} True if the text was found and removed, false otherwise
     */
    remove(str) {
        const currentText = op_smudgy_get_current_line();
        const index = currentText.indexOf(str);
        if (index !== -1) {
            this.removeAt(index, index + str.length);
        }
    },

    /**
     * Prevents the current line from being displayed (gags it completely)
     */
    gag() {
        op_smudgy_gag();
    },


    get text() {
        return op_smudgy_get_current_line();
    },

    get number() {
        return op_smudgy_get_current_line_number();
    }
};


/**
 * Global buffer manipulation object for modifying existing lines
 * 
 * @namespace buffer
 */
const buffer = {
    /**
     * Inserts text at the specified position with optional styling
     * @param {number} line_number - The line number to insert at
     * @param {string} text - The text to insert
     * @param {number} begin - The start position to insert at
     * @param {number} [end] - The end position (for replacement), defaults to begin
     * @param {Object} [options] - Styling options
     * @param {string|Object} [options.fg] - Foreground color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     * @param {string|Object} [options.bg] - Background color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     */
    insert(line_number, text, begin, end = begin, options = {}) {
        op_smudgy_line_insert(line_number, text, begin, end, options.fg || null, options.bg || null);
    },

    /**
     * Replaces text in the specified range
     * @param {number} line_number - The line number to replace at
     * @param {string} text - The replacement text
     * @param {number} begin - The start position to replace
     * @param {number} end - The end position to replace
     */
    replaceAt(line_number, text, begin, end) {
        op_smudgy_line_replace(line_number, text, begin, end);
    },

    /**
     * Highlights text in the specified range with the given colors
     * @param {number} line_number - The line number to highlight at
     * @param {number} begin - The start position to highlight
     * @param {number} end - The end position to highlight
     * @param {Object} [options] - Styling options
     * @param {string|Object} [options.fg] - Foreground color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     * @param {string|Object} [options.bg] - Background color (string like "red" or RGB object {r,g,b} or ANSI object {color,bold})
     */
    highlightAt(line_number, begin, end, options = {}) {
        op_smudgy_line_highlight(line_number, begin, end, options.fg || null, options.bg || null);
    },

    /**
     * Removes text in the specified range
     * @param {number} line_number - The line number to remove at
     * @param {number} begin - The start position to remove
     * @param {number} end - The end position to remove
     */
    removeAt(line_number, begin, end) {
        op_smudgy_line_remove(line_number, begin, end);
    },
};
Object.defineProperty(globalThis, "line", { value: line });
Object.defineProperty(globalThis, "buffer", { value: buffer });
Object.defineProperty(globalThis, "createAlias", { value: createAlias });
Object.defineProperty(globalThis, "createTrigger", { value: createTrigger });
Object.defineProperty(globalThis, "createTriggers", { value: createTriggers });
