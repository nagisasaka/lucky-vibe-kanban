import React,{useState} from 'react';import {createRoot} from 'react-dom/client';import {QueryClient,QueryClientProvider} from '@tanstack/react-query';
import {useSessionSend} from '../../packages/web-core/src/features/workspace-chat/model/hooks/useSessionSend';
import {usePendingChatMessages,appendPendingMessages} from '../../packages/web-core/src/features/workspace-chat/model/pendingChatMessages';
const api=window as any;api.entries=[];api.calls=[];api.followUp=async(sessionId:string,body:any)=>{api.calls.push({sessionId,body});return await new Promise((resolve,reject)=>{api.ack=resolve;api.reject=reject})};
function Harness(){const [session,setSession]=useState('session');const [version,setVersion]=useState(0);api.render=()=>setVersion(v=>v+1);api.setSession=setSession;
 const send=useSessionSend({sessionId:session,workspaceId:'workspace',isNewSessionMode:!session,executorConfig:{executor:'CODEX',variant:null,model:null}} as any);api.send=send;
 const messages=usePendingChatMessages(s=>s.messages);api.pending=messages;api.store=usePendingChatMessages;
 const rows=appendPendingMessages(api.entries,messages);return <div data-version={version}>{rows.map(e=><div key={e.patchKey} data-state={e.pendingDelivery?.state??'canonical'}>{e.type==='NORMALIZED_ENTRY'?e.content.content:''}</div>)}<div role="alert">{send.error}</div></div>
}
createRoot(document.getElementById('root')!).render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><Harness/></QueryClientProvider>);
