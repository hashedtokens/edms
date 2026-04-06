import React, { useState } from 'react';
import { Navbar } from '../layout/Navbar';
import { Footer } from '../layout/Footer';
import { TestSidebar } from '../testview/TestSidebar';
import { TestControls } from '../testview/TestControls';
import { TagManager } from '../common/TagManager';
import { AnnotationInput } from '../common/AnnotationInput';
import { JsonEditor } from '../common/JsonEditor';
import { RequestResponsePanels } from '../testview/RequestResponsePanels';

export const TestView = () => {
  const [showWarning, setShowWarning] = useState(false);
  const [history, setHistory] = useState([]);
  const [bookmarks, setBookmarks] = useState([]);
  const [showBookmarks, setShowBookmarks] = useState(false);
  const [method, setMethod] = useState('POST');
  const [url, setUrl] = useState('');
  const [request, setRequest] = useState('');
  const [response, setResponse] = useState('Waiting for response...');
  const [isLoading, setIsLoading] = useState(false);
  const [tags, setTags] = useState([]);
  const [annotation, setAnnotation] = useState('');
  const [methodFilter, setMethodFilter] = useState('ALL');
  const [searchQuery, setSearchQuery] = useState('');
  const [dateSearch, setDateSearch] = useState('');
  const [endpoints, setEndpoints] = useState([]);
  const [isLoadingEndpoints, setIsLoadingEndpoints] = useState(false);
  const [loadTime, setLoadTime] = useState(null);
  const [showEndpoints, setShowEndpoints] = useState(false);

  const handleRequestChange = (text) => {
    setRequest(text);
    const wordCount = text.trim().split(/\s+/).filter(word => word.length > 0).length;
    setShowWarning(wordCount > 1000);
  };

  const addTag = (tag) => {
    setTags([...tags, tag]);
  };

  const removeTag = (index) => {
    setTags(tags.filter((_, i) => i !== index));
  };

  const handleTest = () => {
    if (!url) {
      setResponse('Please enter an API endpoint URL');
      return;
    }

    setIsLoading(true);
    
    setTimeout(() => {
      try {
        let requestData = {};
        if (request.trim()) {
          requestData = JSON.parse(request);
        }

        const echoResponse = {
          message: 'Echo response',
          receivedData: requestData,
          method: method,
          endpoint: url,
          timestamp: new Date().toISOString()
        };

        setResponse(JSON.stringify(echoResponse, null, 2));
        const newHistoryItem = {
          id: Date.now(),
          method: method,
          url: url,
          request: request,
          response: JSON.stringify(echoResponse, null, 2),
          timestamp: new Date().toISOString(),
          tags: [...tags],
          annotation: annotation,
          isBookmarked: false
        };
        
        setHistory(prev => [newHistoryItem, ...prev]);

        // Reset tags and annotation after test
        setTags([]);
        setAnnotation('');

      } catch (error) {
        setResponse(`Error: Invalid JSON - ${error.message}`);
      } finally {
        setIsLoading(false);
      }
    }, 500);
  };

  const loadEndpoints = async () => {
    setIsLoadingEndpoints(true);
    setLoadTime(null);
    
    try {
      const startTime = performance.now();
      // Simulated API call
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      const dummyEndpoints = [];
      const categories = ['users', 'products', 'orders', 'auth', 'settings'];
      const methods = ['GET', 'POST', 'PUT', 'DELETE'];
      
      for (let i = 1; i <= 20; i++) {
        const category = categories[Math.floor(Math.random() * categories.length)];
        const method = methods[Math.floor(Math.random() * methods.length)];
        
        dummyEndpoints.push({
          id: i,
          method: method,
          path: `/api/${category}/${i}`,
          category: category,
          status: 'active',
          responseTime: Math.floor(Math.random() * 500) + 100,
          description: `${method} endpoint for ${category} management`,
          dummyRequest: { id: i, action: 'test' },
          dummyResponse: { success: true, data: { id: i, message: 'Response from dummy endpoint' } }
        });
      }
      
      const endTime = performance.now();
      
      setEndpoints(dummyEndpoints);
      setLoadTime({
        server: 1000,
        client: Math.round(endTime - startTime)
      });
      setShowEndpoints(true);
    } catch (error) {
      console.error('Error loading endpoints:', error);
      setResponse(`Error loading endpoints: ${error.message}`);
    } finally {
      setIsLoadingEndpoints(false);
    }
  };

  const loadEndpointData = (endpoint) => {
    setMethod(endpoint.method);
    setUrl(`http://localhost:3001${endpoint.path}`);
    setRequest(JSON.stringify(endpoint.dummyRequest, null, 2));
    setResponse(JSON.stringify(endpoint.dummyResponse, null, 2));
  };

  const toggleBookmark = (id) => {
    setHistory(prev => {
      const updatedHistory = prev.map(item => {
        if (item.id === id) {
          return { ...item, isBookmarked: !item.isBookmarked };
        }
        return item;
      });
      
      const bookmarkedItems = updatedHistory.filter(item => item.isBookmarked);
      setBookmarks(bookmarkedItems);
      
      return updatedHistory;
    });
  };

  const loadFromHistory = (item) => {
    setMethod(item.method);
    setUrl(item.url);
    setRequest(item.request);
    setResponse(item.response);
    setTags(item.tags || []);
    setAnnotation(item.annotation || '');
  };

  const filterItems = (items) => {
    return items.filter(item => {
      const matchesMethod = methodFilter === 'ALL' || item.method === methodFilter;
      const matchesSearch = searchQuery === '' || 
        (item.tags && item.tags.some(tag => tag.toLowerCase().includes(searchQuery.toLowerCase()))) ||
        (item.url && item.url.toLowerCase().includes(searchQuery.toLowerCase())) ||
        (showEndpoints && item.path.toLowerCase().includes(searchQuery.toLowerCase()));
      
      const matchesDate = dateSearch === '' || 
        (item.timestamp && new Date(item.timestamp).toLocaleDateString() === new Date(dateSearch).toLocaleDateString());
      
      return matchesMethod && matchesSearch && matchesDate;
    });
  };

  const copyToClipboard = (text) => {
    navigator.clipboard.writeText(text);
  };

  return (
    <div className="min-h-screen flex flex-col">
      <Navbar />
      
      <div className="flex flex-1 bg-gray-50 pt-20">
        <TestSidebar
          showBookmarks={showBookmarks}
          showEndpoints={showEndpoints}
          setShowBookmarks={setShowBookmarks}
          setShowEndpoints={setShowEndpoints}
          bookmarks={bookmarks}
          endpoints={endpoints}
          history={history}
          methodFilter={methodFilter}
          setMethodFilter={setMethodFilter}
          searchQuery={searchQuery}
          setSearchQuery={setSearchQuery}
          dateSearch={dateSearch}
          setDateSearch={setDateSearch}
          loadEndpoints={loadEndpoints}
          isLoadingEndpoints={isLoadingEndpoints}
          loadTime={loadTime}
          toggleBookmark={toggleBookmark}
          loadFromHistory={loadFromHistory}
          loadEndpointData={loadEndpointData}
          filterItems={filterItems}
        />

        {/* Main Content */}
        <div className="flex-1 overflow-y-auto bg-gray-50">
          <div className="p-6 max-w-7xl mx-auto">
            {showWarning && (
              <div className="fixed top-24 left-1/2 transform -translate-x-1/2 z-50 animate-slideDown">
                <div className="bg-white border-2 border-yellow-500 rounded-lg p-4 flex items-center gap-3 shadow-lg">
                  <span className="text-2xl">⚠️</span>
                  <span className="text-yellow-800 font-medium text-sm">
                    Large request detected! Response time may be delayed due to the size of your request.
                  </span>
                  <button 
                    className="text-yellow-800 text-xl p-1 hover:bg-yellow-100 rounded"
                    onClick={() => setShowWarning(false)}
                  >
                    ✕
                  </button>
                </div>
              </div>
            )}

            <TestControls
              method={method}
              setMethod={setMethod}
              url={url}
              setUrl={setUrl}
              onTest={handleTest}
              onStop={() => console.log('Stop')}
              onSave={() => console.log('Save')}
              onDownload={() => console.log('Download')}
              onCopyUrl={() => copyToClipboard(url)}
              isLoading={isLoading}
            />

            <div className="flex gap-3 mb-4">
              <select 
                className="px-4 py-2 border border-gray-300 rounded bg-white text-gray-700 w-32 outline-none"
                value={method}
                onChange={(e) => setMethod(e.target.value)}
              >
                <option>POST</option>
                <option>PUT</option>
                <option>GET</option>
                <option>DELETE</option>
              </select>
              <input
                type="text"
                placeholder="Enter API endpoint URL"
                className="flex-1 px-4 py-2 border border-gray-300 rounded text-gray-700 outline-none focus:border-purple-500 focus:ring-2 focus:ring-purple-200"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
              />
              <button 
                className="px-6 py-2 bg-white border border-gray-300 rounded cursor-pointer hover:bg-gray-50"
                onClick={() => copyToClipboard(url)}
              >
                📋
              </button>
            </div>

            <AnnotationInput
              annotation={annotation}
              setAnnotation={setAnnotation}
              placeholder="Annotation per endpoint"
              maxWords={250}
            />

            <TagManager
              tags={tags}
              onAddTag={addTag}
              onRemoveTag={removeTag}
              maxTags={25}
              placeholder="Add tags (press Enter or click +)"
            />

            <div className="grid grid-cols-2 gap-6">
              <JsonEditor
                title="Request (JSON Editor)"
                value={request}
                onChange={handleRequestChange}
                height="288px"
                showActions={true}
              />
              
              <JsonEditor
                title="Response (JSON Editor)"
                value={response}
                readOnly={true}
                height="288px"
                showActions={true}
              />
            </div>
          </div>
        </div>
      </div>

      {/* <RequestResponsePanels
  request={request}
  response={response}
  onRequestChange={handleRequestChange}
/> */}

      <Footer />
    </div>
  );
};