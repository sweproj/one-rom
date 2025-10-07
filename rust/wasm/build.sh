#!/bin/bash
wasm-pack build --target web
echo ""
echo -n "One ROM wasm binary size: "
ls -lh pkg/onerom_wasm_bg.wasm | awk '{print $5}'
