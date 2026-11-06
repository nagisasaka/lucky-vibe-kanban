import React, {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {LexicalComposer} from '@lexical/react/LexicalComposer';
import {PlainTextPlugin} from '@lexical/react/LexicalPlainTextPlugin';
import {ContentEditable} from '@lexical/react/LexicalContentEditable';
import {LexicalErrorBoundary} from '@lexical/react/LexicalErrorBoundary';
import {useLexicalComposerContext} from '@lexical/react/LexicalComposerContext';
import {$getSelection,$isRangeSelection,KEY_ENTER_COMMAND} from 'lexical';
import {TRANSFORMERS} from '@lexical/markdown';
import {HistoryPlugin} from '@lexical/react/LexicalHistoryPlugin';
import {MarkdownSyncPlugin} from '../../packages/ui/src/components/MarkdownSyncPlugin';
import {PlainTextPastePlugin} from '../../packages/ui/src/components/PlainTextPastePlugin';
import {KeyboardCommandsPlugin} from '../../packages/ui/src/components/KeyboardCommandsPlugin';
import {TypeaheadOpenProvider} from '../../packages/ui/src/components/TypeaheadOpenContext';
import {createDecoratorNode} from '../../packages/ui/src/components/create-decorator-node';
import {$getNodeByKey} from 'lexical';
import type {TextMatchTransformer} from '@lexical/markdown';
const attachment=createDecoratorNode<{label:string;src:string}>({type:'fixture-attachment',serialization:{format:'inline',pattern:/(?<!!)\[([^\]]+)\]\((\.lvk\/attachments\/[^)]+)\)/,trigger:')',serialize:d=>`[${d.label}](${d.src})`,deserialize:m=>({label:m[1],src:m[2]})},component:({data,nodeKey})=>{const[editor]=useLexicalComposerContext();return <button onClick={()=>editor.update(()=>$getNodeByKey(nodeKey)?.remove())}>Remove {data.label}</button>}});
const allTransformers=[...attachment.transformers,...TRANSFORMERS];
const api=window as any;
api.sent=[];api.files=[];
function Capture(){
 const [editor]=useLexicalComposerContext();api.editor=editor;
 api.enter=(composing=false)=>editor.dispatchCommand(KEY_ENTER_COMMAND,new KeyboardEvent('keydown',{key:'Enter',isComposing:composing,cancelable:true}));
 api.oldFormat=()=>editor.update(()=>{const s=$getSelection();if($isRangeSelection(s))s.setFormat('code')});
 return null;
}
function App(){
 const [value,setValue]=useState('');api.value=value;api.restore=setValue;
 return <LexicalComposer initialConfig={{namespace:'literal-chat',nodes:[attachment.Node],onError:e=>{throw e}}}>
  <Capture/><HistoryPlugin/>
  <MarkdownSyncPlugin plainText plainTextAttachments={attachment.transformers as TextMatchTransformer[]} value={value} onChange={setValue} transformers={allTransformers} editable />
  <PlainTextPlugin contentEditable={<ContentEditable style={{whiteSpace:'pre-wrap'}} aria-label="Chat input" onPasteCapture={e=>{if(e.clipboardData.files.length){e.preventDefault();e.stopPropagation();api.files.push(...Array.from(e.clipboardData.files).map(f=>f.name))}}}/>} placeholder={null} ErrorBoundary={LexicalErrorBoundary}/>
  <PlainTextPastePlugin/>
  <TypeaheadOpenProvider><KeyboardCommandsPlugin plainText transformers={allTransformers} onChange={setValue} sendShortcut="Enter" onCmdEnter={()=>{api.sent.push(value);setValue('')}}/></TypeaheadOpenProvider>
 </LexicalComposer>
}
createRoot(document.getElementById('root')!).render(<App/>);
