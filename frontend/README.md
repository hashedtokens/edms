# EDMS Interface Redesign

## Description

This project contains the redesigned frontend prototypes for the EDMS (Endpoint Management System).

The redesign focuses on endpoint organization, inspection, filtering, testing, and management, while maintaining a modular architecture for future backend integration.

The current implementation is built using **HTML, Tailwind CSS, and JavaScript**. The frontend is served through **NGINX in Docker** for local testing.

The application includes the following views:

- **Home**
- **Test View**
- **Bookmark View**
- **Collection View**
- **Repo View**

---

## Installation

Clone the repository:

```bash
git clone <repository-url>
```

Navigate to the project directory:

```bash
cd <project-directory>
```

## Usage

### Local

Open the required prototype page in a browser.

### Docker

Create the shared network if it does not already exist:

```bash
docker network create hashedtokens-edms
```

Build the frontend image:

```bash
docker build -t edms-frontend .
```

Run the frontend:

```bash
docker run -d --name frontend-edms --network hashedtokens-edms -p 3911:80 edms-frontend
```

Then open:

```
http://localhost:3911
```

The backend should also be connected to the `hashedtokens-edms` Docker network for frontend-to-backend communication.

## Features

### List View

- Endpoint listing
- Dynamic JSON-based endpoint data
- Collection/folder-based endpoint loading
- Endpoint search
- Endpoint filtering
- Multiple endpoint tags
- Endpoint selection
- QP selection and highlighting
- Dynamic QP data
- Request and Response panels
- Improved toolbar
- Resizable columns
- Pagination
- Tags Manager for renaming and merging bookmark tags

### Collection View

- Dedicated collection management interface
- Consistent navigation with other EDMS views
- Collection search
- Collection filtering
- Collection tags
- Active folder selection
- Folder-based endpoint organization
- New empty collection creation
- Folder rename
- Folder duplication
- Folder deletion
- Collection merge workflow
- Endpoint selection during collection merging
- Data View for opening a selected collection in List View
- Import and Export workflow foundation

### Test View

The Test View is specifically focused on testing endpoints and managing the resulting history and bookmarks.

- Endpoint testing
- History of tested endpoints
- Bookmark management
- Existing endpoint-QP pair selection
- QP selection
- Request and Response panels
- Separate Headers and Body views
- Run and Stop controls
- Search and filtering
- Time and URL filters
- Endpoint information and metadata
- History data management
- Bookmarking tested endpoints
- RWR request/response workflow
- Backend RWR workflow independently verified

### Repo View

- Repository view listing
- Repository search
- CRUD-type filtering
- Tag filtering
- Segment filtering
- Repository selection
- Pagination
- CRUD counts
- Data size
- EID count
- Segment count
- Data tag count
- QP count
- Index list count
- Annotation information

### Navigation

- Shared navigation across all views
- Active page highlighting
- Reusable navigation component
- Modular JavaScript implementation

### Deployment

- Multi-stage Docker build
- Node build stage for Tailwind CSS
- NGINX serving stage
- NGINX configuration for frontend serving
- Shared Docker network for frontend/backend communication

### General

- Responsive UI
- HTML + Tailwind CSS + JavaScript
- Modular JavaScript architecture
- Shared reusable components
- Dynamic endpoint data
- Collection-based endpoint organization
- Backend-ready structure

## Known Issues & Tips

- **NGINX blank/404 page:** check the root redirect and ensure the built paths are correct.
- **Tailwind styles missing:** ensure `npm run build:css` runs during the Docker build.
- **Frontend changes not appearing:** rebuild the Docker image after making changes.
- **Backend connection issues:** ensure the frontend and backend containers are connected to the same Docker network.

## Roadmap

Current priorities:

- Backend API integration
- RWR integration with Test View
- Real Request/Response data
- History management
- Bookmark management
- Static Website preview
- Dashboard/Home improvements
- Further endpoint management functionality
- TypeScript migration
- UI/UX refinement

## Contributing

- Development should be carried out in the appropriate working branch.
- Code changes should be submitted through a Merge Request before merging into the development branch.

## Authors

Developed as part of the EDMS project.

## Project Status

🚧 **Active Development**

The frontend redesign, shared navigation, core interactions, Collection View, List View, Test View, Repo View, and frontend Dockerization have been implemented.

The RWR workflow has been independently verified.

Current work focuses on backend API integration, particularly integrating the verified RWR workflow into Test View, along with further UI/UX refinement.