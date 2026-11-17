# Options window for convert3d on macOS and Linux.
# Usage: options_dialog.py <input.json> <output.json>
# Exit 0 after Save, exit 2 if the window is closed without saving.

import json
import sys

try:
    import tkinter as tk
    from tkinter import filedialog, ttk
except Exception as exc:  # noqa: BLE001
    print(f"tkinter is required to open Convert 3D Options ({exc})", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    with open(sys.argv[1], encoding="utf-8") as f:
        data = json.load(f)

    root = tk.Tk()
    root.title("Convert 3D Options")
    root.geometry("980x800")
    root.minsize(820, 620)

    ask = tk.BooleanVar(value=bool(data.get("ask_output")))
    tk.Checkbutton(
        root,
        text="Let user specify the output folder before any conversion",
        variable=ask,
        anchor="w",
    ).pack(fill="x", padx=12, pady=(10, 0))

    hint = (
        "When this is unchecked, the converted file is saved in the same folder as the original.\n\n"
        "One step is one application reading a format and writing another. With Maya, .ma to .usda is "
        "Maya (writes .fbx) then Blender (writes .usda). Aztek reads and writes .ma, .mb, .obj, .fbx and "
        "USD itself, so that conversion is one step. Paths below are shared. A path on one file type "
        "replaces only that read or write. Any other program is run as: program.exe input output. "
        "Clear a field to use the shared application again."
    )
    tk.Label(root, text=hint, justify="left", wraplength=940).pack(fill="x", padx=12, pady=8)

    existing = tk.StringVar(value=data.get("existing") or "ask")
    exist = ttk.LabelFrame(root, text="When the output already exists")
    exist.pack(fill="x", padx=12, pady=4)
    ttk.Radiobutton(exist, text="Ask before conversion", variable=existing, value="ask").pack(anchor="w", padx=8)
    ttk.Radiobutton(exist, text="Always overwrite", variable=existing, value="overwrite").pack(anchor="w", padx=8)
    ttk.Radiobutton(exist, text="Increment the name (file_(2))", variable=existing, value="increment").pack(
        anchor="w", padx=8
    )
    ttk.Label(exist, text="With a custom folder, both questions are the same window.").pack(
        anchor="w", padx=8, pady=(0, 6)
    )

    apps = ttk.LabelFrame(root, text="Applications")
    apps.pack(fill="x", padx=12, pady=4)
    tool_vars = {}
    format_vars = []

    def browse(var: tk.StringVar) -> None:
        path = filedialog.askopenfilename(parent=root)
        if path:
            var.set(path)

    def sync(var: tk.StringVar, previous: dict) -> None:
        old = previous["value"]
        new = var.get()
        if old and old != new:
            for other in format_vars:
                if other.get() == old:
                    other.set(new)
        previous["value"] = new

    for row, tool in enumerate(data.get("tools") or []):
        ttk.Label(apps, text=tool.get("label") or tool["id"]).grid(row=row, column=0, sticky="w", padx=8, pady=3)
        var = tk.StringVar(value=tool.get("path") or "")
        previous = {"value": var.get()}
        tool_vars[tool["id"]] = var
        entry = ttk.Entry(apps, textvariable=var, width=80)
        entry.grid(row=row, column=1, sticky="ew", padx=4)
        entry.bind("<FocusOut>", lambda _e, v=var, p=previous: sync(v, p))
        ttk.Button(apps, text="...", width=3, command=lambda v=var, p=previous: (browse(v), sync(v, p))).grid(
            row=row, column=2, padx=4
        )
    apps.columnconfigure(1, weight=1)

    frame = ttk.Frame(root)
    frame.pack(fill="both", expand=True, padx=12, pady=6)
    canvas = tk.Canvas(frame, highlightthickness=0)
    scroll = ttk.Scrollbar(frame, orient="vertical", command=canvas.yview)
    body = ttk.Frame(canvas)
    body.bind("<Configure>", lambda e: canvas.configure(scrollregion=canvas.bbox("all")))
    window = canvas.create_window((0, 0), window=body, anchor="nw")
    canvas.configure(yscrollcommand=scroll.set)
    canvas.bind("<Configure>", lambda e: canvas.itemconfigure(window, width=e.width))
    canvas.pack(side="left", fill="both", expand=True)
    scroll.pack(side="right", fill="y")

    ttk.Label(body, text="File type").grid(row=0, column=0, sticky="w", padx=4)
    ttk.Label(body, text="Read").grid(row=0, column=1, sticky="w", padx=4)
    ttk.Label(body, text="Write").grid(row=0, column=3, sticky="w", padx=4)
    body.columnconfigure(1, weight=1)
    body.columnconfigure(3, weight=1)

    rows = []
    for i, row in enumerate(data.get("rows") or [], start=1):
        ttk.Label(body, text=f"{row['name']} (.{row['ext']})").grid(row=i, column=0, sticky="w", padx=4, pady=2)
        read = tk.StringVar(value=row.get("read") or "")
        write = tk.StringVar(value=row.get("write") or "")
        read_entry = ttk.Entry(body, textvariable=read, width=36)
        write_entry = ttk.Entry(body, textvariable=write, width=36)
        read_entry.grid(row=i, column=1, sticky="ew", padx=4, pady=2)
        ttk.Button(body, text="...", width=3, command=lambda v=read: browse(v)).grid(row=i, column=2)
        write_entry.grid(row=i, column=3, sticky="ew", padx=4, pady=2)
        ttk.Button(body, text="...", width=3, command=lambda v=write: browse(v)).grid(row=i, column=4)
        if not row.get("can_read", True):
            read_entry.state(["disabled"])
        else:
            format_vars.append(read)
        if not row.get("can_write", True):
            write_entry.state(["disabled"])
        else:
            format_vars.append(write)
        rows.append((row["ext"], read, write))

    saved = {"ok": False}

    def save() -> None:
        out = {
            "ask_output": bool(ask.get()),
            "existing": existing.get(),
            "tools": [{"id": key, "path": var.get().strip()} for key, var in tool_vars.items()],
            "rows": [{"ext": ext, "read": read.get().strip(), "write": write.get().strip()} for ext, read, write in rows],
        }
        with open(sys.argv[2], "w", encoding="utf-8") as f:
            json.dump(out, f)
        saved["ok"] = True
        root.destroy()

    buttons = ttk.Frame(root)
    buttons.pack(fill="x", padx=12, pady=10)
    ttk.Button(buttons, text="Cancel", command=root.destroy).pack(side="right")
    ttk.Button(buttons, text="Save", command=save).pack(side="right", padx=6)
    root.protocol("WM_DELETE_WINDOW", root.destroy)
    root.mainloop()
    sys.exit(0 if saved["ok"] else 2)


if __name__ == "__main__":
    main()
