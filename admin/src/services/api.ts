import { createApi, fetchBaseQuery } from '@reduxjs/toolkit/query/react';

export interface ApiConfig {
  id: string;
  name: string;
  version: string;
  datasource_name: string;
  schemas?: any[];
  created_at: string;
}

export interface DataSource {
  id: string;
  name: string;
  db_type: string;
  host: string;
  port: number;
  database: string;
  username: string;
}

export interface Listener {
  id: string;
  name: string;
  host: string;
  port: number;
  api_configs: string[];
}

export const apiSlice = createApi({
  reducerPath: 'api',
  baseQuery: fetchBaseQuery({
    baseUrl: '/apify/admin',
    prepareHeaders: (headers) => {
      // Get API key from localStorage
      const apiKey = localStorage.getItem('apiKey');
      if (apiKey) {
        headers.set('X-API-KEY', apiKey);
      }
      return headers;
    },
  }),
  tagTypes: ['Apis', 'DataSources', 'Listeners', 'Schemas'],
  endpoints: (builder) => ({
    // API Configs
    getApis: builder.query<ApiConfig[], void>({
      query: () => '/apis',
      providesTags: ['Apis'],
    }),
    getApi: builder.query<ApiConfig, string>({
      query: (id) => `/apis/${id}`,
      providesTags: ['Apis'],
    }),
    createApi: builder.mutation<ApiConfig, Partial<ApiConfig>>({
      query: (body) => ({
        url: '/apis',
        method: 'POST',
        body,
      }),
      invalidatesTags: ['Apis'],
    }),
    updateApi: builder.mutation<ApiConfig, { id: string; data: Partial<ApiConfig> }>({
      query: ({ id, data }) => ({
        url: `/apis/${id}`,
        method: 'PUT',
        body: data,
      }),
      invalidatesTags: ['Apis'],
    }),
    deleteApi: builder.mutation<void, string>({
      query: (id) => ({
        url: `/apis/${id}`,
        method: 'DELETE',
      }),
      invalidatesTags: ['Apis'],
    }),

    // Data Sources
    getDataSources: builder.query<DataSource[], void>({
      query: () => '/datasources',
      providesTags: ['DataSources'],
    }),
    createDataSource: builder.mutation<DataSource, Partial<DataSource>>({
      query: (body) => ({
        url: '/datasources',
        method: 'POST',
        body,
      }),
      invalidatesTags: ['DataSources'],
    }),
    deleteDataSource: builder.mutation<void, string>({
      query: (id) => ({
        url: `/datasources/${id}`,
        method: 'DELETE',
      }),
      invalidatesTags: ['DataSources'],
    }),

    // Listeners
    getListeners: builder.query<Listener[], void>({
      query: () => '/listeners',
      providesTags: ['Listeners'],
    }),
    createListener: builder.mutation<Listener, Partial<Listener>>({
      query: (body) => ({
        url: '/listeners',
        method: 'POST',
        body,
      }),
      invalidatesTags: ['Listeners'],
    }),
    deleteListener: builder.mutation<void, string>({
      query: (id) => ({
        url: `/listeners/${id}`,
        method: 'DELETE',
      }),
      invalidatesTags: ['Listeners'],
    }),
  }),
});

export const {
  useGetApisQuery,
  useGetApiQuery,
  useCreateApiMutation,
  useUpdateApiMutation,
  useDeleteApiMutation,
  useGetDataSourcesQuery,
  useCreateDataSourceMutation,
  useDeleteDataSourceMutation,
  useGetListenersQuery,
  useCreateListenerMutation,
  useDeleteListenerMutation,
} = apiSlice;
