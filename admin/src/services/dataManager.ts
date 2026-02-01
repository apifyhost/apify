import { apiSlice } from './api';

export interface TableRow {
  [key: string]: any;
  id?: string | number;
}

export interface TableSchema {
  tableName: string;
  columns: {
    name: string;
    columnType: string;
    nullable: boolean;
    primaryKey: boolean;
    autoIncrement?: boolean;
    defaultValue?: any;
  }[];
}

export interface QueryParams {
  limit?: number;
  offset?: number;
  where?: Record<string, any>;
}

export const dataManagerApi = apiSlice.injectEndpoints({
  endpoints: (builder) => ({
    listTables: builder.query<string[], string>({
      query: (datasource) => `/data/${datasource}/tables`,
      providesTags: (result, _error, datasource) => 
        result ? [{ type: 'Tables' as const, id: datasource }] : [],
    }),
    getTableSchema: builder.query<TableSchema, { datasource: string; table: string }>({
      query: ({ datasource, table }) => `/data/${datasource}/schema/${table}`,
    }),
    queryTable: builder.query<TableRow[], { datasource: string; table: string; params?: QueryParams }>({
      query: ({ datasource, table, params }) => ({
        url: `/data/${datasource}/${table}/query`,
        method: 'POST',
        body: params || {},
      }),
      providesTags: (_result, _error, { datasource, table }) => 
        [{ type: 'TableData' as const, id: `${datasource}:${table}` }],
    }),
    createRecord: builder.mutation<any, { datasource: string; table: string; data: any }>({
      query: ({ datasource, table, data }) => ({
        url: `/data/${datasource}/${table}`,
        method: 'POST',
        body: data,
      }),
      invalidatesTags: (_result, _error, { datasource, table }) => 
        [{ type: 'TableData' as const, id: `${datasource}:${table}` }],
    }),
    updateRecord: builder.mutation<any, { datasource: string; table: string; id: string | number; data: any }>({
      query: ({ datasource, table, id, data }) => ({
        url: `/data/${datasource}/${table}/${id}`,
        method: 'PUT',
        body: data,
      }),
      invalidatesTags: (_result, _error, { datasource, table }) => 
        [{ type: 'TableData' as const, id: `${datasource}:${table}` }],
    }),
    deleteRecord: builder.mutation<any, { datasource: string; table: string; id: string | number }>({
      query: ({ datasource, table, id }) => ({
        url: `/data/${datasource}/${table}/${id}`,
        method: 'DELETE',
      }),
      invalidatesTags: (_result, _error, { datasource, table }) => 
        [{ type: 'TableData' as const, id: `${datasource}:${table}` }],
    }),
  }),
});

export const {
  useListTablesQuery,
  useGetTableSchemaQuery,
  useQueryTableQuery,
  useCreateRecordMutation,
  useUpdateRecordMutation,
  useDeleteRecordMutation,
} = dataManagerApi;
