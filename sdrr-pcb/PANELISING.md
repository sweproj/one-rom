# PANELISING PCBs

WORK IN PROGRESS - DO NOT USE

## Setup

Install kikit into the KiCad python environment:

```zsh
/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3 -m pip install kikit
```

add to .zshrc
```
alias kikit='/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3 -m kikit.ui'
```

```zsh
source ~/.zshrc
```

## Usage

1. Generate the panel using kikit:

    ```zsh
    mkdir -p panels/fire-24-d-5x4-panel
    kikit panelize -p support/fire-24-d-5x4-panel-config.json verified/fire-24-d/kicad/fire-24-d.kicad_pcb panels/fire-24-d-5x4-panel/fire-24-d-5x4-panel.kicad_pcb
    ```

2. Manually add "CNC" on Edge Cuts in spaces that should be milled out.

3. Export gerbers (do not refill zones) and drill files.  It's best to export to a sub-directory.  Create a zip file of the contents of that directory for submission.

4. Upload the gerbers.

5. Select colour.

6. Select Panel by Customer and the number of columns/rows.

7. Ensure Remove Mark selected.

8. Select Confirm Production file.

9. Select PCB Assembly

Use the standard bom and pos files.