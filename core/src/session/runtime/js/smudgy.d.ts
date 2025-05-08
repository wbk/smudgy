/**
 * Creates an alias that matches patterns and executes a script
 * @param {string} name - The name of the alias
 * @param {string|RegExp|Array<string|RegExp>} patterns - The pattern(s) to match
 * @param {string|Function} script - The script to execute when the alias matches
 * @returns {Alias} The created alias
 */
declare function createAlias(name: string, patterns: string | RegExp | Array<string | RegExp>, script: string | Function): Alias;
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
declare function createTriggers(triggers: {
    [x: string]: TriggerDef;
}): {
    [x: string]: Trigger;
};
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
declare function createTrigger(name: string, patterns: string | RegExp | TriggerPatterns, script: string | Function, options?: TriggerOptions): Trigger;
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
declare function validateCreateTriggerParams(name: string, patterns: string | RegExp | TriggerPatterns, script: string | Function, options: TriggerOptions): any;
/**
 * Normalizes the patterns for a trigger
 * @private
 * @param {String|RegExp|TriggerPatterns} patterns - The pattern(s) to normalize
 * @returns {Object} An object containing normalized patterns, raw patterns, and anti-patterns
 */
declare function normalizePatterns(patterns: string | RegExp | TriggerPatterns): any;
declare const op_smudgy_get_current_session: any;
declare const op_smudgy_get_session_character: any;
declare const op_smudgy_get_sessions: any;
declare const op_smudgy_create_simple_alias: any;
declare const op_smudgy_create_javascript_function_alias: any;
declare const op_smudgy_create_simple_trigger: any;
declare const op_smudgy_create_javascript_function_trigger: any;
declare const op_smudgy_set_alias_enabled: any;
declare const op_smudgy_set_trigger_enabled: any;
declare const op_smudgy_session_echo: any;
declare const op_smudgy_session_reload: any;
declare const op_smudgy_session_send: any;
declare const op_smudgy_session_send_raw: any;
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
declare class Session {
    constructor(id: any);
    _id: any;
    set id(value: string);
    /**
     * Gets the ID of the session.
     *
     * @returns {string} The ID of the session.
     */
    get id(): string;
    /**
     * Echoes a line of text to the terminal
     *
     * @param {string} line - The text to echo.
     */
    echo(line: string): void;
    /**
     * Reloads the session's scripts.
     */
    reload(): void;
    /**
     * Sends a line of text to the MUD server, which will be processed exactly as if it were typed by the user.
     *
     * @param {string} line - The text to send.
     */
    send(line: string): void;
    /**
     * Sends a raw line of text to the MUD server without any processing.
     *
     * @param {string} line - The raw text to send.
     */
    sendRaw(line: string): void;
    /**
     * Gets the character associated with this session.
     *
     * @returns {Character} The character name.
     */
    get character(): Character;
    /**
     * Returns a string representation of the Session object.
     *
     * @returns {string} A string representation of the Session.
     */
    toString(): string;
}
declare class Alias {
    constructor(name: any);
    name: any;
    set enabled(value: any);
}
declare class Trigger {
    constructor(name: any);
    name: any;
    set enabled(value: any);
}
/**
 * Sends a line of text to the current session.
 * @param {string} line - The line of text to send
 */
declare function send(line: string): void;
/**
 * Sends a line of text to the current session without any processing.
 * @param {string} line - The raw line of text to send
 */
declare function sendRaw(line: string): void;
declare function echo(line: any): void;
type TriggerDef = {
    /**
     * - The pattern(s) to match.
     */
    patterns?: (string | RegExp)[];
    /**
     * - The raw pattern(s) to match
     */
    rawPatterns?: (string | RegExp)[];
    /**
     * - The anti-pattern(s) to exclude
     */
    antiPatterns?: (string | RegExp)[];
    /**
     * - The script to execute when the trigger matches
     */
    script: string | Function;
    /**
     * - Should this trigger fire when a prompt is received, in addition to firing when new lines are received
     */
    prompt?: boolean;
    /**
     * - Should this trigger be enabled by default
     */
    enabled?: boolean;
};
type TriggerPatterns = {
    /**
     * - The pattern(s) to match.
     */
    patterns?: (string | RegExp)[];
    /**
     * - The raw pattern(s) to match
     */
    rawPatterns?: (string | RegExp)[];
    /**
     * - The anti-pattern(s) to exclude
     */
    antiPatterns?: (string | RegExp)[];
};
type TriggerOptions = {
    /**
     * - (Default: false) Should this trigger fire when a prompt is received, in addition to firing when new lines are received
     */
    prompt?: boolean;
    /**
     * - (Default: true) Should this trigger be enabled by default
     */
    enabled?: boolean;
};
type Character = {
    name?: string;
    subtext?: string;
};
//# sourceMappingURL=smudgy.d.ts.map