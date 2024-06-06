
session -> connection -> vt_preprocessor -> trigger_processor -> script_runtime -> buffer -> view
   |-----> keypress_input ---------> hotkey_processor----.
   |-----> input -> alias_processor <-> script_runtime <-'
                                          | 
                                          | (echoed output)
                                               | 
                                               '- buffer -> view
