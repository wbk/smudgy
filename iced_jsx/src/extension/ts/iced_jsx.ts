
declare const Deno: {
    core: {
        ops: any
    };
};

const {
    op_iced_jsx_create_widget,
    op_iced_jsx_remove_widget,
    op_iced_jsx_create_view_fn_vec,
    op_iced_jsx_push_to_view_fn_vec,
    op_iced_jsx_create_column,
    op_iced_jsx_create_progress_bar,
    op_iced_jsx_create_row,
    op_iced_jsx_create_text,
} = (Deno as any).core.ops;


const IcedJsx = {
    setWidget: (name: string, widget: any) => {
        op_iced_jsx_create_widget(name, widget);
    },

    removeWidget: (name: string) => {
        op_iced_jsx_remove_widget(name);
    },

    createElement: function (type: any, props: Record<string, any>, ...children: any) {
        if (typeof type === "function") {
            return type(props || {}, children.flat());
        } else {
            throw new Error("Invalid type");
        }
    },

    Column: (props: Record<string, any>, children: any) => {
        const viewFnVec = op_iced_jsx_create_view_fn_vec();
        if (children.length > 0) {
            children.forEach((child: any) => {
                op_iced_jsx_push_to_view_fn_vec(viewFnVec, child);
            });
        } else {
            op_iced_jsx_push_to_view_fn_vec(viewFnVec, children);
        }
        return op_iced_jsx_create_column(viewFnVec, props.width, props.height, props.spacing, props.padding);
    },

    Row: (props: Record<string, any>, children: any) => {
        const viewFnVec = op_iced_jsx_create_view_fn_vec();
        if (children.length > 0) {
            children.forEach((child: any) => {
                op_iced_jsx_push_to_view_fn_vec(viewFnVec, child);
            });
        } else {
            op_iced_jsx_push_to_view_fn_vec(viewFnVec, children);
        }
        return op_iced_jsx_create_row(viewFnVec, props.width, props.height, props.spacing, props.padding);
    },

    Label: (props: Record<string, any>, children: any) => {
        return op_iced_jsx_create_text(children.join(""), props.color || "");
    },

    ProgressBar: (props: Record<string, any>, children: any) => {
        return op_iced_jsx_create_progress_bar(props);
    }
}

Object.defineProperty(globalThis, "IcedJsx", { value: IcedJsx });
Object.defineProperty(globalThis, "React", { value: {createElement: IcedJsx.createElement} });
