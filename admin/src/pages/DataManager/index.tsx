import { useState, useEffect } from 'react';
import {
  Layout,
  Select,
  Menu,
  Table,
  Button,
  Space,
  Empty,
  Typography,
  Drawer,
  Form,
  Input,
  InputNumber,
  Switch,
  message,
  Popconfirm,
} from 'antd';
import {
  DatabaseOutlined,
  TableOutlined,
  PlusOutlined,
  ReloadOutlined,
  EditOutlined,
  DeleteOutlined,
} from '@ant-design/icons';
import { useGetDataSourcesQuery } from '@/services/api';
import {
  useListTablesQuery,
  useGetTableSchemaQuery,
  useQueryTableQuery,
  useCreateRecordMutation,
  useUpdateRecordMutation,
  useDeleteRecordMutation,
} from '@/services/dataManager';

const { Header, Sider, Content } = Layout;
const { Title } = Typography;
const { Option } = Select;

export const DataManagerPage = () => {
  const [selectedDs, setSelectedDs] = useState<string | null>(null);
  const [selectedTable, setSelectedTable] = useState<string | null>(null);
  
  // Queries
  const { data: datasources = [], isLoading: isLoadingDs } = useGetDataSourcesQuery();
  const { data: rawTables = [] } = useListTablesQuery(selectedDs || '', {
    skip: !selectedDs,
  });

  const tables = rawTables.filter(t => !t.startsWith('_meta_'));
  
  // Set default DS
  useEffect(() => {
    if (datasources.length > 0 && !selectedDs) {
      setSelectedDs(datasources[0].name);
    }
  }, [datasources, selectedDs]);

  // Set default Table
  useEffect(() => {
    if (tables.length > 0 && !selectedTable) {
      setSelectedTable(tables[0]);
    } else if (tables.length === 0) {
        setSelectedTable(null);
    }
  }, [tables, selectedTable]);


  return (
    <Layout style={{ height: 'calc(100vh - 64px)' }}>
      <Header style={{ background: '#fff', padding: '0 16px', borderBottom: '1px solid #f0f0f0', display: 'flex', alignItems: 'center', gap: 16 }}>
        <DatabaseOutlined style={{ fontSize: 18 }} />
        <Select
          style={{ width: 200 }}
          placeholder="选择数据源"
          value={selectedDs}
          onChange={setSelectedDs}
          loading={isLoadingDs}
        >
          {datasources.map((ds) => (
            <Option key={ds.id} value={ds.name}>
              {ds.name} ({ds.db_type})
            </Option>
          ))}
        </Select>
      </Header>
      <Layout>
        <Sider width={200} theme="light" style={{ borderRight: '1px solid #f0f0f0' }}>
          {selectedDs ? (
            <Menu
              mode="inline"
              selectedKeys={selectedTable ? [selectedTable] : []}
              onClick={({ key }) => setSelectedTable(key)}
              items={tables.map((table) => ({
                key: table,
                icon: <TableOutlined />,
                label: table,
              }))}
              style={{ borderRight: 0 }}
            />
          ) : (
             <Empty description="请先选择数据源" style={{ marginTop: 24 }} />
          )}
        </Sider>
        <Content style={{ padding: 24, overflow: 'auto' }}>
          {selectedDs && selectedTable ? (
            <TableView datasource={selectedDs} table={selectedTable} />
          ) : (
            <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%' }}>
              <Empty description={selectedDs ? "请选择数据表" : "请先选择数据源"} />
            </div>
          )}
        </Content>
      </Layout>
    </Layout>
  );
};

// Subcomponent for Table View
const TableView = ({ datasource, table }: { datasource: string; table: string }) => {
  const [form] = Form.useForm();
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);
  const [editingRecord, setEditingRecord] = useState<any>(null);

  const { data: schema, isLoading: isLoadingSchema } = useGetTableSchemaQuery({ datasource, table });
  const { data: tableData = [], isLoading: isLoadingData, refetch } = useQueryTableQuery({ datasource, table });
  
  const [createRecord, { isLoading: isCreating }] = useCreateRecordMutation();
  const [updateRecord, { isLoading: isUpdating }] = useUpdateRecordMutation();
  const [deleteRecord] = useDeleteRecordMutation();

  const handleAdd = () => {
    setEditingRecord(null);
    form.resetFields();
    setIsDrawerOpen(true);
  };

  const handleEdit = (record: any) => {
    setEditingRecord(record);
    // Convert dates if needed, simple mapping for now
    form.setFieldsValue(record);
    setIsDrawerOpen(true);
  };

  const handleDelete = async (id: any) => {
    try {
      await deleteRecord({ datasource, table, id }).unwrap();
      message.success('删除成功');
    } catch (err: any) {
        message.error('删除失败: ' + (err.data?.message || err.message));
    }
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      if (editingRecord) {
        // Assume 'id' is key for now. TODO: Use PK from schema
        const pk = schema?.columns.find(c => c.primaryKey)?.name || 'id';
        await updateRecord({ datasource, table, id: editingRecord[pk], data: values }).unwrap();
        message.success('更新成功');
      } else {
        await createRecord({ datasource, table, data: values }).unwrap();
        message.success('创建成功');
      }
      setIsDrawerOpen(false);
      form.resetFields();
    } catch (err: any) {
         message.error('操作失败: ' + (err.data?.message || err.message));
    }
  };

  // Generate Table Columns from Schema
  const columns = schema?.columns.map((col) => {
      // Don't show password or sensitive fields by default if naming convention matches? 
      // For now just show all.
      return {
          title: col.name,
          dataIndex: col.name,
          key: col.name,
          render: (text: any) => {
              if (typeof text === 'boolean') {
                  return <Switch checked={text} disabled size="small" />;
              }
              if (typeof text === 'object' && text !== null) {
                  return JSON.stringify(text);
              }
              return text;
          }
      };
  }) || [];

  // Add Action Column
  if (columns.length > 0) {
      columns.push({
          title: '操作',
          key: 'action',
          fixed: 'right',
          width: 150,
          render: (_: any, record: any) => (
             <Space>
                 <Button type="link" icon={<EditOutlined />} onClick={() => handleEdit(record)}>编辑</Button>
                 <Popconfirm 
                    title="确定删除?" 
                    onConfirm={() => {
                        // Find PK
                        const pk = schema?.columns.find(c => c.primaryKey)?.name || 'id';
                        handleDelete(record[pk]);
                    }}
                 >
                     <Button type="link" danger icon={<DeleteOutlined />}>删除</Button>
                 </Popconfirm>
             </Space>
          )
      } as any);
  }

  // Generate Form Items from Schema
  const renderFormItems = () => {
      return schema?.columns.map(col => {
          if (col.autoIncrement || (col.primaryKey && col.defaultValue === null && !editingRecord)) {
              // Skip auto-increment PKs in create mode, show as disabled in edit?
              // Simple logic: if primary key and auto_increment, hide from form
               if (col.autoIncrement) return null;
          }

          let inputComponent = <Input />;
          let valuePropName = 'value';
          
          if (col.columnType.includes('INT') || col.columnType.includes('NUMERIC') || col.columnType.includes('FLOAT')) {
              inputComponent = <InputNumber style={{ width: '100%' }} />;
          } else if (col.columnType.includes('BOOL') || col.columnType.includes('TINYINT(1)')) { // SQLite bools
               inputComponent = <Switch />;
               valuePropName = 'checked';
          } else if (col.columnType.includes('TEXT') && !col.columnType.includes('VARCHAR')) {
               inputComponent = <Input.TextArea rows={4} />;
          } else if (col.columnType.includes('DATETIME') || col.columnType.includes('TIMESTAMP')) {
               // Handle dates if we had a date library hookup for values, but string fallback is safer for generic
               // inputComponent = <DatePicker showTime />;
          }

          return (
              <Form.Item 
                key={col.name} 
                name={col.name} 
                label={col.name} 
                valuePropName={valuePropName}
                rules={[{ required: !col.nullable && !col.defaultValue, message: '必填项' }]}
              >
                  {inputComponent}
              </Form.Item>
          );
      });
  };

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between' }}>
        <Title level={4} style={{ margin: 0 }}>表: {table}</Title>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={() => refetch()}>刷新</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleAdd}>添加记录</Button>
        </Space>
      </div>
      
      <Table 
         dataSource={tableData} 
         columns={columns} 
         rowKey={(record) => {
             const pk = schema?.columns.find(c => c.primaryKey)?.name || 'id';
             return record[pk] || Math.random();
         }}
         loading={isLoadingSchema || isLoadingData}
         scroll={{ x: 'max-content' }}
      />

      <Drawer
        title={editingRecord ? "编辑记录" : "添加记录"}
        width={480}
        onClose={() => setIsDrawerOpen(false)}
        open={isDrawerOpen}
        extra={
            <Space>
                <Button onClick={() => setIsDrawerOpen(false)}>取消</Button>
                <Button type="primary" onClick={handleSubmit} loading={isCreating || isUpdating}>提交</Button>
            </Space>
        }
      >
          <Form form={form} layout="vertical">
             {renderFormItems()}
          </Form>
      </Drawer>
    </div>
  );
};
