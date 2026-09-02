"""Ultra-minimalist design system and theme for fototriagem."""

# Palette: Deep obsidian, neutral grays, subtle slate/blue accents
COLOR_BG_DARK = "#09090b"        # Deepest neutral obsidian
COLOR_BG_SURFACE = "#121215"     # Subtle elevated surface
COLOR_BG_CARD = "#18181b"        # Component card background
COLOR_BG_SELECTED = "#27272a"    # Active list selection
COLOR_ACCENT = "#3b82f6"         # Clean blue accent
COLOR_ACCENT_HOVER = "#60a5fa"
COLOR_BORDER = "#27272a"         # Muted border
COLOR_TEXT_PRIMARY = "#f4f4f5"   # Crisp white/zinc
COLOR_TEXT_SECONDARY = "#a1a1aa" # Muted gray
COLOR_TEXT_MUTED = "#71717a"     # Subtle hints
COLOR_AMBER = "#f59e0b"          # Duplicate warning
COLOR_RED = "#ef4444"            # Trash / danger

FONT_FAMILY = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif"
FONT_MONO = "'JetBrains Mono', 'Fira Code', 'SF Mono', Consolas, monospace"

WINDOW_STYLE = f"""
QMainWindow {{
    background-color: {COLOR_BG_DARK};
}}
QWidget {{
    font-family: {FONT_FAMILY};
    color: {COLOR_TEXT_PRIMARY};
}}
"""

STATUS_BAR_STYLE = f"""
QWidget#bottom_bar {{
    background-color: {COLOR_BG_SURFACE};
    border-top: 1px solid {COLOR_BORDER};
}}
QLabel#status_pos {{
    font-size: 13px;
    font-weight: 600;
    color: {COLOR_TEXT_PRIMARY};
    padding-right: 8px;
}}
QLabel#status_name {{
    font-size: 13px;
    font-weight: 500;
    color: {COLOR_TEXT_PRIMARY};
}}
QLabel#status_date {{
    font-size: 12px;
    color: {COLOR_TEXT_SECONDARY};
}}
QLabel#status_tag {{
    font-family: {FONT_MONO};
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    background-color: #1e293b;
    color: #94a3b8;
    border: 1px solid #334155;
}}
QLabel#status_copies {{
    font-size: 11px;
    font-weight: 500;
    color: {COLOR_AMBER};
    padding: 2px 6px;
    border-radius: 4px;
    background-color: rgba(245, 158, 11, 0.12);
    border: 1px solid rgba(245, 158, 11, 0.3);
}}
QLabel#status_hints {{
    font-size: 11px;
    color: {COLOR_TEXT_MUTED};
}}
"""

MONTH_VIEW_STYLE = f"""
QWidget#month_container {{
    background-color: {COLOR_BG_DARK};
}}
QLabel#header_title {{
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.5px;
    color: {COLOR_TEXT_PRIMARY};
}}
QLabel#header_subtitle {{
    font-size: 13px;
    color: {COLOR_TEXT_SECONDARY};
}}
QListWidget#month_list {{
    background-color: transparent;
    border: none;
    outline: none;
}}
QListWidget#month_list::item {{
    background-color: {COLOR_BG_CARD};
    border: 1px solid {COLOR_BORDER};
    border-radius: 8px;
    padding: 14px 18px;
    margin-bottom: 8px;
    font-size: 14px;
    color: {COLOR_TEXT_PRIMARY};
}}
QListWidget#month_list::item:selected {{
    background-color: {COLOR_BG_SELECTED};
    border: 1px solid {COLOR_ACCENT};
    color: #ffffff;
}}
QLabel#summary_text {{
    color: {COLOR_TEXT_MUTED};
    font-size: 12px;
    line-height: 1.4;
}}
QLabel#footer_hints {{
    color: {COLOR_TEXT_MUTED};
    font-size: 12px;
    border-top: 1px solid {COLOR_BORDER};
    padding-top: 12px;
}}
"""

MENU_DIALOG_STYLE = f"""
QDialog {{
    background-color: {COLOR_BG_SURFACE};
    border: 1px solid {COLOR_BORDER};
    border-radius: 12px;
}}
QLabel#menu_title {{
    font-size: 15px;
    font-weight: 600;
    color: {COLOR_TEXT_PRIMARY};
    padding-bottom: 6px;
    border-bottom: 1px solid {COLOR_BORDER};
}}
QLineEdit#menu_input {{
    background-color: {COLOR_BG_DARK};
    border: 1px solid {COLOR_ACCENT};
    border-radius: 6px;
    color: #ffffff;
    padding: 10px 12px;
    font-size: 14px;
    font-family: {FONT_FAMILY};
}}
QListWidget#menu_list {{
    background-color: transparent;
    border: none;
    outline: none;
}}
QListWidget#menu_list::item {{
    padding: 10px 14px;
    border-radius: 6px;
    color: {COLOR_TEXT_SECONDARY};
    font-size: 13px;
    margin: 2px 0;
}}
QListWidget#menu_list::item:selected {{
    background-color: {COLOR_BG_SELECTED};
    color: #ffffff;
    font-weight: 500;
}}
QLabel#menu_hints {{
    font-size: 11px;
    color: {COLOR_TEXT_MUTED};
    border-top: 1px solid {COLOR_BORDER};
    padding-top: 8px;
}}
"""
