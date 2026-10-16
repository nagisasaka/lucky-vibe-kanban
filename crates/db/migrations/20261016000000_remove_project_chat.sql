-- Retire the internal project assistant. Keep the original migration immutable.
-- Workspace/MCP question reservations and command admission remain shared runtime state.
DROP TRIGGER project_chat_issue_created;
DROP TRIGGER project_chat_workspace_link;
DROP TRIGGER project_chat_direct_merge;
DROP TRIGGER project_chat_run_requested;
DROP TRIGGER project_chat_agent_event;
DROP TRIGGER project_chat_issue_update;
DROP TRIGGER project_chat_direct_command;
DROP TRIGGER project_chat_question_answer;

DROP TABLE project_chat_operations;
DROP TABLE project_chat_messages;
DROP TABLE project_chat_events;
DROP TABLE project_conversations;
