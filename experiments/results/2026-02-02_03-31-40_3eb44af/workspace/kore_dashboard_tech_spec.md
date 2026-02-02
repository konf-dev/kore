Okay, let's break down the technical specifications for the Kore Agentic Framework Dashboard, building upon the architectural considerations.

## Kore Agentic Framework Dashboard: Detailed Technical Specification

### 1. Introduction

This document outlines the detailed technical specifications for the Kore Agentic Framework Dashboard. The dashboard provides a comprehensive interface for managing, monitoring, and interacting with Kore agents. Its primary goal is to offer visibility into agent behavior, facilitate configuration, and enable effective human-agent collaboration.

### 2. High-Level Architecture (Recap)

The architecture follows a standard client-server model with a clear separation of concerns.

*   **Frontend (Dashboard UI):** A Single Page Application (SPA) providing the user interface.
*   **Backend (API Server):** A robust API layer serving data and orchestrating interactions with Kore, the database, and other services.
*   **Database:** Persistent storage for dashboard-specific data (config, history, human input).
*   **Kore Agent Runtime:** The existing Kore framework, which the backend will interact with.

### 3. Data Models

We'll define data models using a schema-first approach, suitable for both SQL (via ORM) and NoSQL (document structure).

#### 3.1. User
*   `id`: UUID (Primary Key)
*   `username`: String (Unique)
*   `email`: String (Unique)
*   `password_hash`: String
*   `role`: Enum (`admin`, `contributor`, `viewer`)
*   `created_at`: DateTime
*   `updated_at`: DateTime

#### 3.2. Agent Configuration
*   `id`: UUID (Primary Key)
*   `agent_name`: String (Unique, Indexed)
*   `description`: Text
*   `owner_user_id`: UUID (Foreign Key to `User.id`)
*   `agent_definition_path`: String (e.g., path to YAML config, Git repo URL, or embedded definition)
*   `agent_definition`: JSONB/Text (Embedded definition)
*   `environment_variables`: JSONB (Key-value pairs for agent's runtime env)
*   `status`: Enum (`draft`, `deployed`, `archived`)
*   `version`: String (Semantic versioning, e.g., "1.0.0")
*   `created_at`: DateTime
*   `updated_at`: DateTime

#### 3.3. Agent Instance (Runtime Representation)
*   `id`: UUID (Primary Key)
*   `agent_config_id`: UUID (Foreign Key to `AgentConfiguration.id`)
*   `instance_id`: String (Unique identifier from Kore runtime, if Kore exposes one)
*   `deployment_status`: Enum (`pending`, `running`, `stopped`, `error`)
*   `current_kore_pid`: Integer (If Kore runs as a local process, for basic monitoring)
*   `started_at`: DateTime
*   `last_heartbeat_at`: DateTime
*   `error_message`: Text (If in error state)
*   `metadata`: JSONB (Additional runtime-specific info)

#### 3.4. Interaction Log Entry
*   `id`: UUID (Primary Key)
*   `agent_instance_id`: UUID (Foreign Key to `AgentInstance.id`)
*   `timestamp`: DateTime
*   `source`: Enum (`human`, `agent`, `tool`, `system`)
*   `type`: Enum (`message`, `action_request`, `action_response`, `observation`, `thought`, `error`)
*   `content`: Text (Main message/output)
*   `payload`: JSONB (Structured data, e.g., tool arguments, raw observation)
*   `interaction_flow_id`: UUID (To group related steps in a single "turn" or task execution)
*   `tool_used_id`: UUID (Optional, Foreign Key to `ToolDefinition.id` if a tool was executed)

#### 3.5. Task/Goal Tracking
*   `id`: UUID (Primary Key)
*   `agent_instance_id`: UUID (Foreign Key to `AgentInstance.id`)
*   `user_id`: UUID (Optional, if initiated by a specific user)
*   `title`: String
*   `description`: Text
*   `status`: Enum (`pending`, `in_progress`, `completed`, `failed`, `cancelled`)
*   `priority`: Enum (`low`, `medium`, `high`)
*   `start_time`: DateTime
*   `end_time`: DateTime (Actual or estimated)
*   `outcome_summary`: Text
*   `parent_task_id`: UUID (Self-referencing for sub-tasks)
*   `metadata`: JSONB (For linking to external systems, e.g., Jira ticket ID)

#### 3.6. Tool Definition
*   `id`: UUID (Primary Key)
*   `tool_name`: String (Unique)
*   `description`: Text
*   `schema`: JSONB (OpenAPI/JSON schema for tool input/output)
*   `execution_type`: Enum (`local_script`, `http_webhook`, `kore_native_plugin`, `aws_lambda`, `azure_function`)
*   `execution_config`: JSONB (e.g., script path, URL, AWS ARN, API key)
*   `owner_user_id`: UUID (Foreign Key to `User.id`)
*   `created_at`: DateTime
*   `updated_at`: DateTime

#### 3.7. Agent-Tool Connection
*   `id`: UUID (Primary Key)
*   `agent_config_id`: UUID (Foreign Key to `AgentConfiguration.id`)
*   `tool_definition_id`: UUID (Foreign Key to `ToolDefinition.id`)
*   `enabled`: Boolean (Default: True)
*   `parameters_override`: JSONB (Optional, to override default tool parameters for a specific agent)

#### 3.8. Human Input Request
*   `id`: UUID (Primary Key)
*   `interaction_log_entry_id`: UUID (Foreign Key to `InteractionLogEntry.id`, points to agent's request)
*   `agent_instance_id`: UUID (For quick lookup)
*   `requested_at`: DateTime
*   `status`: Enum (`pending`, `fulfilled`, `cancelled`, `timed_out`)
*   `prompt`: Text (What the agent is asking)
*   `expected_format_schema`: JSONB (Optional, JSON schema for expected human input structure)
*   `response_value`: JSONB/Text (Actual human response)
*   `responded_by_user_id`: UUID (Foreign Key to `User.id`)
*   `responded_at`: DateTime
*   `cancellation_reason`: Text

### 4. API Endpoints

We'll primarily use a RESTful API, with WebSockets for real-time updates.

#### 4.1. Core API (REST)

**Authentication:** JWT or OAuth2 bearer tokens. All endpoints require authentication unless specified.

**Prefix:** `/api/v1`

**Users Management:**
*   `POST /users`: Register a new user (admin only or self-registration).
*   `GET /users`: List all users (admin only).
*   `GET /users/{id}`: Get user details.
*   `PUT /users/{id}`: Update user details (admin or self).
*   `DELETE /users/{id}`: Delete user (admin only).
*   `POST /auth/login`: Authenticate and get JWT.
*   `POST /auth/refresh`: Refresh JWT.

**Agent Configuration:**
*   `POST /agents`: Create a new agent configuration.
    *   Body: `AgentConfiguration` object (excluding `id`, `created_at`, `updated_at`).
*   `GET /agents`: List all agent configurations (filterable by `owner_user_id`, `status`).
*   `GET /agents/{id}`: Get a specific agent configuration.
*   `PUT /agents/{id}`: Update an agent configuration.
*   `DELETE /agents/{id}`: Delete an agent configuration.
*   `POST /agents/{id}/deploy`: Deploy an agent configuration (creates an `AgentInstance`).
*   `POST /agents/{id}/undeploy`: Undeploy an `AgentInstance` associated with this config.
*   `GET /agents/{id}/instances`: List active instances for a configuration.

**Agent Instances:**
*   `GET /instances`: List all running agent instances (filterable by `agent_config_id`, `deployment_status`).
*   `GET /instances/{id}`: Get details of a specific agent instance.
*   `POST /instances/{id}/stop`: Gracefully stop an agent instance.
*   `POST /instances/{id}/restart`: Restart an agent instance.
*   `POST /instances/{id}/pause`: Pause an agent instance (if Kore supports).
*   `POST /instances/{id}/resume`: Resume a paused agent instance.

**Interaction Logs:**
*   `GET /instances/{id}/logs`: Retrieve interaction logs for an agent instance (filterable by `timestamp`, `source`, `type`).
*   `GET /logs/{id}`: Get a specific log entry.

**Task Tracking:**
*   `GET /instances/{id}/tasks`: List tasks associated with an agent instance.
*   `GET /tasks/{id}`: Get details of a specific task.
*   `PUT /tasks/{id}`: Update task status/details (e.g., by an admin for oversight).

**Tool Management:**
*   `POST /tools`: Create a new tool definition.
*   `GET /tools`: List all tool definitions.
*   `GET /tools/{id}`: Get a specific tool definition.
*   `PUT /tools/{id}`: Update a tool definition.
*   `DELETE /tools/{id}`: Delete a tool definition.
*   `POST /agents/{agent_id}/tools`: Link a tool to an agent (create `AgentToolConnection`).
*   `PUT /agents/{agent_id}/tools/{tool_id}`: Update specific connection parameters.
*   `DELETE /agents/{agent_id}/tools/{tool_id}`: Unlink a tool from an agent.

**Human-in-the-Loop:**
*   `GET /instances/{id}/human_requests`: List pending human input requests for an agent instance.
*   `GET /human_requests/{id}`: Get details of a specific human input request.
*   `POST /human_requests/{id}/respond`: Submit human input to an agent.
    *   Body: `{ "response_value": "..." }`
*   `POST /human_requests/{id}/cancel`: Cancel a pending human input request.

#### 4.2. Real-time Communication (WebSockets)

**Endpoint:** `wss://yourdashboard.com/ws`

**Channels/Topics (examples):**
*   `/agents/{agent_instance_id}/logs`: Stream new `InteractionLogEntry` for a specific agent instance.
*   `/agents/{agent_instance_id}/status`: Stream `AgentInstance` status updates.
*   `/agents/{agent_instance_id}/human_requests`: Stream new `HumanInputRequest` events for an instance.
*   `/admin/system_events`: Global system events (e.g., new agent deployed, critical error).

**Message Format (JSON):**
```json
{
    "type": "log_entry", // or "status_update", "human_request"
    "timestamp": "ISO_DATETIME",
    "payload": {
        // Data model for the specific type (e.g., InteractionLogEntry without ID)
    }
}
```

### 5. Technology Stack Choices

#### 5.1. Frontend

*   **Framework:** **React.js** (alternatives: Vue.js, Angular). Chosen for its widespread adoption, component-based architecture, and rich ecosystem.
*   **State Management:** **React Query** for server state management (data fetching, caching, invalidation) and **Zustand** or **Jotai** for local UI state. Avoids Redux boilerplate.
*   **UI Library:** **Chakra UI** or **Tailwind CSS + Headless UI**. Provides a robust component library for rapid development and consistent UI.
*   **Routing:** **React Router Dom**.
*   **Charting/Data Visualization:** **Recharts** or **Nivo**.
*   **Build Tool:** **Vite** (faster than Webpack for development).
*   **Language:** **TypeScript**. Essential for maintainability and catching errors early in a complex application.
*   **Deployment:** Static hosting (Netlify, Vercel, AWS S3 + CloudFront).

#### 5.2. Backend

*   **Language:** **Python** (aligns with Kore's likely Python ecosystem).
*   **Framework:** **FastAPI**. Chosen for its high performance (ASGI), automatic OpenAPI/Swagger documentation, modern Python type hints, and simplicity. (Alternatives: Django Rest Framework for larger projects, Flask for microservices).
*   **ORM:** **SQLAlchemy 2.0** with **Alembic** for migrations. Provides a powerful and flexible way to interact with SQL databases.
*   **WebSocket Library:** Built-in FastAPI websockets module or an external library like `websockets`.
*   **Asynchronous Tasks/Job Queue:** **Celery** with **Redis** as a broker. For long-running tasks like agent deployment, large log processing, or scheduled tasks.
*   **Configuration Management:** `python-dotenv` for local `.env` files, environment variables for production.
*   **API Documentation:** FastAPI's auto-generated OpenAPI.

#### 5.3. Database

*   **Primary Database:** **PostgreSQL**. Chosen for its robustness, reliability, support for JSONB (for flexible schema data like `metadata`, `environment_variables`, `payload`, `schema`), excellent indexing capabilities, and transactional integrity.
*   **Cache/Message Broker:** **Redis**. Used for:
    *   Caching frequently accessed data (e.g., agent configurations).
    *   As a message broker for Celery.
    *   Storing real-time session data for WebSockets.

#### 5.4. Integration with Kore Agent Runtime

This is a critical aspect. The specifics depend heavily on how Kore exposes its functionality.

**Option A: Kore as a Library/Embeddable Component (Ideal)**
*   The Backend directly imports Kore libraries.
*   Backend functions call Kore APIs (e.g., `kore.deploy_agent(config)`, `kore.send_message(instance_id, message)`).
*   Kore runtime can register "callbacks" or "listeners" with the dashboard backend (e.g., on `log_event`, `human_input_request`, `tool_execution_start/end`). This is the most efficient.

**Option B: Kore as a Separate Microservice/Process (More likely)**
*   **Deployment & Control:** Backend manages Kore processes (e.g., using `subprocess` for local deployments for testing, or orchestrator like Kubernetes for production). Sends configuration.
*   **Communication Protocols:**
    *   **REST API (from Kore):** Kore exposes its own REST API for status, log retrieval, sending messages, and receiving commands. The Dashboard Backend consumes this API.
    *   **WebSockets (from Kore to Dashboard):** Kore connects to the Dashboard's WebSocket endpoint to push real-time events (logs, internal thoughts, tool calls, human input requests).
    *   **Message Queue (Pub/Sub):** Kore publishes events to a message queue (e.g., Kafka, RabbitMQ, Redis Pub/Sub), and the Dashboard backend subscribes to relevant topics. This is generally more scalable and resilient.
*   **Agent Deployment Handlers:** The `POST /agents/{id}/deploy` endpoint needs to:
    1.  Provision resources for the Kore agent (e.g., spin up a Docker container).
    2.  Inject the agent configuration and environment variables.
    3.  Configure the Kore agent to report its status and logs back to the Dashboard's API/WebSocket/MQ.
    4.  Store the `AgentInstance` details, including the runtime-specific ID.

### 6. Deployment Considerations

*   **Containerization:** **Docker**. Essential for packaging the frontend (Nginx serving React build), backend (FastAPI application), PostgreSQL, and Redis into portable images.
*   **Orchestration:** **Kubernetes** (alternatives: Docker Swarm, AWS ECS, Azure AKS). For managing, scaling, and ensuring high availability of containers.
    *   **Helm Charts:** To define, install, and upgrade the application in Kubernetes.
*   **Cloud Provider:** AWS, Azure, Google Cloud Platform (GCP).
    *   **Example AWS Stack:**
        *   Frontend: AWS S3 + CloudFront
        *   Backend: AWS EKS (Kubernetes) or AWS Fargate (serverless containers)
        *   Database: AWS RDS PostgreSQL
        *   Cache/Broker: AWS ElastiCache for Redis
        *   Logging: AWS CloudWatch Logs (collected via Fluentd/Fluent Bit in K8s)
        *   Monitoring: AWS CloudWatch Metrics, Prometheus + Grafana (within EKS)
        *   CI/CD: AWS CodePipeline/CodeBuild or GitHub Actions

*   **CI/CD:** Automate builds, tests, and deployments triggered by code commits.
    *   Frontend: Build React app, push to S3.
    *   Backend: Build Docker image, push to container registry (ECR), update Kubernetes deployment.
*   **Environment Variables:** Strict separation of configurations for development, staging, production.
*   **Scalability:**
    *   **Horizontal scaling for Backend:** Multiple FastAPI instances behind a load balancer.
    *   **Auto-scaling for Kubernetes pods:** Based on CPU/memory usage.
    *   **Read Replicas for PostgreSQL:** For read-heavy workloads (esp. log fetching).
    *   **Caching with Redis:** Reduces database load.

### 7. Security Aspects

*   **Authentication & Authorization:**
    *   **JWT (JSON Web Tokens):** For API authentication. Short-lived access tokens, longer-lived refresh tokens.
    *   **Role-Based Access Control (RBAC):