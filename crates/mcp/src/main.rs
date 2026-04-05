mod client;
mod tools;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};

#[derive(Debug, Clone)]
struct BrailleServer;

#[allow(refining_impl_trait_internal)]
impl ServerHandler for BrailleServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        info.server_info = Implementation::new("braille-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Braille is a text-mode browser engine for LLM agents. Use browse_new to create a \
             session, browse_goto to navigate, then interact with click/type/select/snap/eval. \
             Close sessions with browse_close when done."
                .into(),
        );
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: tools::tool_list(),
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Null);
        Ok(tools::call_tool(&request.name, args).await)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = rmcp::transport::io::stdio();
    let server = BrailleServer.serve(transport).await?;
    server.waiting().await?;
    Ok(())
}
