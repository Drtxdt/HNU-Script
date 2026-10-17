export function trapFocus(event:KeyboardEvent,root:HTMLElement){
 if(event.key!=='Tab')return;
 const items=[...root.querySelectorAll<HTMLElement>('button:not([disabled]),a[href],input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex="0"]')].filter(el=>el.getClientRects().length>0);
 const first=items[0],last=items.at(-1);if(!first)return;
 if(event.shiftKey&&document.activeElement===first){event.preventDefault();last?.focus();}else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
}
