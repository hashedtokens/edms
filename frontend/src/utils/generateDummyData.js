export const generateDummyEndpoints = (count = 20) => {
  const methods = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH'];
  const baseUrls = ['/api/users', '/api/products', '/api/orders', '/api/auth', '/api/settings'];
  const tags = ['user', 'admin', 'public', 'private', 'beta', 'deprecated'];
  const annotations = [
    'User authentication endpoint',
    'Product management API',
    'Order processing system',
    'Settings configuration',
    'Data export functionality'
  ];
  
  const dummyEndpoints = [];
  
  for (let i = 1; i <= count; i++) {
    const method = methods[Math.floor(Math.random() * methods.length)];
    const baseUrl = baseUrls[Math.floor(Math.random() * baseUrls.length)];
    const endpointTags = [...new Set(Array(Math.floor(Math.random() * 3) + 1)
      .fill(0)
      .map(() => tags[Math.floor(Math.random() * tags.length)]))];
    
    const numRequests = Math.floor(Math.random() * 5) + 1;
    const requests = [];
    const responses = [];
    
    for (let j = 1; j <= numRequests; j++) {
      requests.push({
        num: j,
        metadata: { size_bytes: Math.floor(Math.random() * 5000) + 100 },
        data: {
          method: method,
          url: `${baseUrl}/${i}`,
          headers: { 'Content-Type': 'application/json' },
          body: method !== 'GET' ? { id: i, data: `Request data ${j}` } : null
        }
      });
      
      responses.push({
        num: j,
        metadata: { size_bytes: Math.floor(Math.random() * 8000) + 200 },
        data: {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
          body: { success: true, data: `Response data ${j}` }
        }
      });
    }
    
    dummyEndpoints.push({
      id: i,
      method: method,
      url: `${baseUrl}/${i}`,
      tags: endpointTags,
      annotation: annotations[Math.floor(Math.random() * annotations.length)],
      date: `2025-01-${String(Math.floor(Math.random() * 28) + 1).padStart(2, '0')}`,
      time: `${String(Math.floor(Math.random() * 12) + 1).padStart(2, '0')}:${String(Math.floor(Math.random() * 60)).padStart(2, '0')} ${Math.random() > 0.5 ? 'AM' : 'PM'}`,
      requests: requests,
      responses: responses
    });
  }
  
  return dummyEndpoints;
};