# One confirmation before a conversion, for macOS and Linux.
# Usage: place_dialog.py <input.json> <output.json>

import json
import os
import sys

try:
    import tkinter as tk
    from tkinter import filedialog, ttk
except Exception as exc:  # noqa: BLE001
    print(f"tkinter is required ({exc})", file=sys.stderr)
    sys.exit(1)


def existing(folder, jobs):
    found = []
    if not folder or not os.path.isdir(folder):
        return found
    for job in jobs:
        for name in (job or {}).get("names") or []:
            if name and os.path.exists(os.path.join(folder, name)) and name not in found:
                found.append(name)
    return found


def main() -> None:
    with open(sys.argv[1], encoding="utf-8") as handle:
        data = json.load(handle)
    ask_folder = bool(data.get("ask_folder"))
    ask_conflict = bool(data.get("ask_conflict"))
    jobs = data.get("jobs") or []

    root = tk.Tk()
    root.title("Convert 3D")
    root.resizable(False, False)
    folder = tk.StringVar(value=data.get("folder") or "")
    choice = tk.StringVar(value="increment")

    if ask_folder:
        ttk.Label(root, text="Save converted files to").pack(anchor="w", padx=12, pady=(12, 0))
        row = ttk.Frame(root)
        row.pack(fill="x", padx=12, pady=4)
        ttk.Entry(row, textvariable=folder, width=48).pack(side="left", fill="x", expand=True)

        def browse() -> None:
            picked = filedialog.askdirectory()
            if picked:
                folder.set(picked)

        ttk.Button(row, text="Browse...", command=browse).pack(side="left", padx=(6, 0))

    warn = ttk.Frame(root)
    text = tk.StringVar(value="")
    ttk.Label(warn, text="⚠", font=("Segoe UI", 16)).grid(row=0, column=0, sticky="n")
    ttk.Label(warn, textvariable=text, justify="left", wraplength=380).grid(row=0, column=1, sticky="w", padx=8)
    if ask_folder:
        radios = ttk.Frame(warn)
        radios.grid(row=1, column=1, sticky="w", pady=(6, 0))
        ttk.Radiobutton(radios, text="Overwrite", variable=choice, value="overwrite").pack(side="left")
        ttk.Radiobutton(radios, text="Increment (file_(2))", variable=choice, value="increment").pack(
            side="left", padx=8
        )

    def refresh(*_args) -> None:
        found = existing(folder.get().strip(), jobs) if ask_conflict else []
        if not found:
            warn.pack_forget()
            return
        shown = ", ".join(found[:3])
        if len(found) > 3:
            shown += f" and {len(found) - 3} more"
        text.set(
            shown + ' already exists.\nOverwrite replaces it. Increment keeps it and adds "_(2)" to the name.'
        )
        warn.pack(fill="x", padx=12, pady=8)

    folder.trace_add("write", refresh)
    refresh()

    def save(force=None) -> None:
        path = folder.get().strip()
        if ask_folder and not path:
            return
        overwrite = force if force is not None else (warn.winfo_ismapped() and choice.get() == "overwrite")
        with open(sys.argv[2], "w", encoding="utf-8") as handle:
            json.dump({"folder": path, "overwrite": bool(overwrite)}, handle)
        root.destroy()

    buttons = ttk.Frame(root)
    buttons.pack(fill="x", padx=12, pady=12)
    ttk.Button(buttons, text="Cancel", command=root.destroy).pack(side="right")
    if ask_folder:
        ttk.Button(buttons, text="Convert", command=save).pack(side="right", padx=6)
    else:
        ttk.Button(buttons, text="Increment", command=lambda: save(False)).pack(side="right", padx=6)
        ttk.Button(buttons, text="Overwrite", command=lambda: save(True)).pack(side="right")
    root.protocol("WM_DELETE_WINDOW", root.destroy)
    root.mainloop()
    sys.exit(0 if os.path.isfile(sys.argv[2]) else 2)


if __name__ == "__main__":
    main()
