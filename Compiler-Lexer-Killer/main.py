if __name__ == "__main__":
    try:
        from compiler_lexer_killer.ui.app import main
        main()
    except Exception:
        # Windowed executables have no stderr: retain a useful startup diagnostic.
        import os
        import traceback
        from pathlib import Path
        report = os.environ.get("LEXER_SMOKE_OUTPUT")
        target = Path(report).with_suffix(".startup-error.log") if report else Path("startup-error.log")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(traceback.format_exc(), encoding="utf-8")
        raise SystemExit(1)
