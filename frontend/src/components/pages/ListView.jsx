import React, { useState } from 'react';
import { Navbar } from '../layout/Navbar';
import { Footer } from '../layout/Footer';
import { BackgroundEffects } from '../layout/BackgroundEffects';
import { SearchBar } from '../common/SearchBar';
import { FilterBar } from '../common/FilterBar';
import { SelectionActions } from '../common/SelectionActions';
import { EndpointTable } from '../listview/EndpointTable';
import { RequestSidebar } from '../listview/RequestSidebar';
import { RequestDetail } from '../listview/RequestDetail';
import { generateDummyEndpoints } from '../../utils/generateDummyData';
import { formatBytes } from '../../utils/formatBytes';

export const ListView = () => {
  const [selectedEndpoints, setSelectedEndpoints] = useState([]);
  const [filterType, setFilterType] = useState('all');
  const [folderName, setFolderName] = useState('');
  const [expandedTags, setExpandedTags] = useState({});
  const [selectedEndpoint, setSelectedEndpoint] = useState(null);
  const [selectedRequestNum, setSelectedRequestNum] = useState(null);
  const [selectedRequests, setSelectedRequests] = useState([]);
  const [endpoints, setEndpoints] = useState([]);
  const [loading, setLoading] = useState(false);
  const [filterInfo, setFilterInfo] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [searchFilter, setSearchFilter] = useState('all');
  const [isSearchApplied, setIsSearchApplied] = useState(false);

  const loadAllEndpoints = async () => {
    setLoading(true);
    try {
      await new Promise(resolve => setTimeout(resolve, 1000));
      const dummyData = generateDummyEndpoints(20);
      setEndpoints(dummyData);
      setFilterInfo('');
      setIsSearchApplied(false);
      setSearchQuery('');
    } catch (error) {
      console.error('Error fetching endpoints:', error);
      alert('Failed to load endpoints.');
    } finally {
      setLoading(false);
    }
  };

  const applySearch = () => {
    if (!searchQuery.trim()) {
      setIsSearchApplied(false);
      return;
    }
    setIsSearchApplied(true);
  };

  const clearSearch = () => {
    setSearchQuery('');
    setIsSearchApplied(false);
  };

  const getFilteredData = () => {
    let filteredEndpoints = endpoints;

    if (isSearchApplied && searchQuery.trim()) {
      const query = searchQuery.toLowerCase().trim();
      
      filteredEndpoints = endpoints.filter(endpoint => {
        switch (searchFilter) {
          case 'method':
            return endpoint.method.toLowerCase().includes(query);
          case 'tags':
            return endpoint.tags.some(tag => tag.toLowerCase().includes(query));
          case 'annotation':
            return endpoint.annotation.toLowerCase().includes(query);
          case 'date':
            return endpoint.date.includes(query) || endpoint.time.toLowerCase().includes(query);
          case 'url':
            return endpoint.url.toLowerCase().includes(query);
          case 'all':
          default:
            return (
              endpoint.method.toLowerCase().includes(query) ||
              endpoint.url.toLowerCase().includes(query) ||
              endpoint.tags.some(tag => tag.toLowerCase().includes(query)) ||
              endpoint.annotation.toLowerCase().includes(query) ||
              endpoint.date.includes(query) ||
              endpoint.time.toLowerCase().includes(query)
            );
        }
      });
    }

    return filteredEndpoints;
  };

  const toggleEndpoint = (id) => {
    if (selectedEndpoints.includes(id)) {
      setSelectedEndpoints(selectedEndpoints.filter(eid => eid !== id));
    } else {
      setSelectedEndpoints([...selectedEndpoints, id]);
    }
  };

  const toggleSelectAll = () => {
    const filteredData = getFilteredData();
    if (selectedEndpoints.length === filteredData.length) {
      setSelectedEndpoints([]);
    } else {
      setSelectedEndpoints(filteredData.map(e => e.id));
    }
  };

  const removeAllSelections = () => {
    setSelectedEndpoints([]);
  };

  const toggleTags = (id) => {
    setExpandedTags(prev => ({
      ...prev,
      [id]: !prev[id]
    }));
  };

  const handleEndpointClick = (id) => {
    setSelectedEndpoint(id);
    setSelectedRequestNum(null);
    setSelectedRequests([]);
  };

  const toggleRequestSelection = (reqNum) => {
    if (selectedRequests.includes(reqNum)) {
      setSelectedRequests(selectedRequests.filter(num => num !== reqNum));
    } else {
      setSelectedRequests([...selectedRequests, reqNum]);
    }
  };

  const handleRequestClick = (reqNum) => {
    setSelectedRequestNum(reqNum);
  };

  const unselectAllRequests = () => {
    setSelectedRequests([]);
  };

  const getTotalRequestSize = (endpoint) => {
    const total = endpoint.requests.reduce((sum, req) => sum + (req.metadata?.size_bytes || 0), 0);
    return formatBytes(total);
  };

  const getTotalResponseSize = (endpoint) => {
    const total = endpoint.responses.reduce((sum, resp) => sum + (resp.metadata?.size_bytes || 0), 0);
    return formatBytes(total);
  };

  const filteredEndpoints = getFilteredData();
  const currentEndpoint = selectedEndpoint ? endpoints.find(e => e.id === selectedEndpoint) : null;
  const currentRequest = currentEndpoint && selectedRequestNum 
    ? currentEndpoint.requests.find(r => r.num === selectedRequestNum) 
    : null;
  const currentResponse = currentEndpoint && selectedRequestNum 
    ? currentEndpoint.responses.find(r => r.num === selectedRequestNum) 
    : null;

  return (
    <div className="list-view-container min-h-screen flex flex-col">
      <BackgroundEffects />
      <Navbar />

      <div className="flex-1 flex flex-col pt-20">
        {/* Top Bar - Fixed */}
        <div className="bg-white border-b border-gray-200">
          <div className="max-w-[1800px] mx-auto px-5 py-4">
            <div className="flex justify-between items-center">
              <div className="flex gap-3 items-center">
                <button className="p-2 bg-white border border-gray-300 rounded cursor-pointer hover:bg-gray-50">🔄</button>
                <button className="px-4 py-2 bg-white border border-gray-300 rounded cursor-pointer hover:bg-gray-50">Save</button>
                <input 
                  type="text"
                  placeholder="folder name"
                  value={folderName}
                  onChange={(e) => setFolderName(e.target.value)}
                  className="px-4 py-2 border border-gray-300 rounded focus:outline-none focus:border-blue-500 w-48"
                />
                <button 
                  className="px-4 py-2 bg-blue-600 text-white border-none rounded cursor-pointer hover:bg-blue-700 disabled:bg-gray-500 disabled:cursor-not-allowed"
                  onClick={loadAllEndpoints}
                  disabled={loading}
                >
                  {loading ? 'Loading...' : 'Load Endpoints'}
                </button>
              </div>
              
              <div className="flex gap-3">
                <button className="px-4 py-2 bg-white border-2 border-gray-800 rounded cursor-pointer hover:bg-gray-50">History</button>
                <button className="px-4 py-2 bg-white border-2 border-gray-800 rounded cursor-pointer hover:bg-gray-50">Bookmarks</button>
                <button className="px-4 py-2 bg-gray-800 text-white border-none rounded cursor-pointer hover:bg-gray-900">⬇</button>
              </div>
            </div>
          </div>
        </div>

        {/* Search Bar */}
        <div className="bg-gray-50 border-b border-gray-200">
          <div className="max-w-[1800px] mx-auto px-5 py-4">
            <SearchBar
              searchQuery={searchQuery}
              setSearchQuery={setSearchQuery}
              searchFilter={searchFilter}
              setSearchFilter={setSearchFilter}
              isSearchApplied={isSearchApplied}
              applySearch={applySearch}
              clearSearch={clearSearch}
              placeholder={`Search by ${searchFilter}...`}
            />
          </div>
        </div>

        {/* Selection Actions */}
        <SelectionActions
          selectedCount={selectedEndpoints.length}
          onRemoveAll={removeAllSelections}
          onBulkTag={() => console.log('Bulk tag')}
          onExport={() => console.log('Export')}
        />

        {/* Main Content Area - Fixed height with flex container */}
        <div className="flex-1 flex overflow-hidden">
          {/* Left Panel - Endpoints Table */}
          <div className="flex-1 flex flex-col min-w-0">
            <div className="bg-white border-b border-gray-200">
              <div className="max-w-full px-5 py-3">
                <FilterBar
                  filterType={filterType}
                  setFilterType={setFilterType}
                />
              </div>
            </div>

            {/* Filter Info Display */}
            {filterInfo && (
              <div className="mx-5 my-4 bg-gray-50 border border-gray-300 rounded-lg overflow-hidden">
                <div className="px-5 py-3 bg-gray-100 border-b border-gray-300 flex justify-between items-center">
                  <h4 className="font-semibold text-gray-700">Filter Information</h4>
                  <button 
                    className="text-gray-600 hover:text-gray-900 text-xl"
                    onClick={() => setFilterInfo('')}
                  >
                    ×
                  </button>
                </div>
                <pre className="p-5 whitespace-pre-wrap font-mono text-sm text-gray-700 max-h-80 overflow-y-auto">
                  {filterInfo}
                </pre>
              </div>
            )}

            {/* Table Container - Scrollable */}
            <div className="flex-1 overflow-auto p-5">
              {endpoints.length === 0 && !loading && (
                <div className="flex justify-center items-center h-full bg-gray-50 rounded-lg border-2 border-dashed border-gray-300">
                  <div className="text-center">
                    <p className="text-gray-600 mb-4">No endpoints loaded.</p>
                    <button 
                      className="px-4 py-2 bg-blue-600 text-white rounded cursor-pointer hover:bg-blue-700"
                      onClick={loadAllEndpoints}
                    >
                      Load Endpoints
                    </button>
                  </div>
                </div>
              )}

              {loading && (
                <div className="flex justify-center items-center h-full">
                  <div className="text-center">
                    <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mb-4"></div>
                    <p className="text-blue-600">Loading endpoints...</p>
                  </div>
                </div>
              )}

              {endpoints.length > 0 && filteredEndpoints.length === 0 && isSearchApplied && (
                <div className="flex flex-col justify-center items-center h-full bg-gray-50 rounded-lg">
                  <p className="text-gray-600 mb-4">No endpoints found matching your search criteria.</p>
                  <button 
                    className="px-4 py-2 bg-gray-600 text-white rounded cursor-pointer hover:bg-gray-700"
                    onClick={clearSearch}
                  >
                    Clear Search
                  </button>
                </div>
              )}

              {filteredEndpoints.length > 0 && (
                <EndpointTable
                  endpoints={filteredEndpoints}
                  selectedEndpoints={selectedEndpoints}
                  toggleEndpoint={toggleEndpoint}
                  toggleSelectAll={toggleSelectAll}
                  expandedTags={expandedTags}
                  toggleTags={toggleTags}
                  onEndpointClick={handleEndpointClick}
                  getTotalRequestSize={getTotalRequestSize}
                  getTotalResponseSize={getTotalResponseSize}
                  filteredEndpoints={filteredEndpoints}
                />
              )}
            </div>
          </div>

          {/* Middle Panel - Requests Sidebar */}
          <div className="w-60 border-l border-gray-200 flex flex-col">
            <RequestSidebar
              currentEndpoint={currentEndpoint}
              selectedRequests={selectedRequests}
              selectedRequestNum={selectedRequestNum}
              onToggleRequest={toggleRequestSelection}
              onRequestClick={handleRequestClick}
              onUnselectAll={unselectAllRequests}
            />
          </div>

          {/* Right Panel - Request/Response Details */}
          <div className="w-96 border-l border-gray-200 flex flex-col">
            <div className="flex-1 flex flex-col overflow-hidden">
              <RequestDetail
                title="Request"
                data={currentRequest}
                requestNum={selectedRequestNum}
                size={currentRequest?.metadata?.size_bytes}
              />
              <RequestDetail
                title="Response"
                data={currentResponse}
                requestNum={selectedRequestNum}
                size={currentResponse?.metadata?.size_bytes}
              />
            </div>
          </div>
        </div>
      </div>

      <Footer />
    </div>
  );
};